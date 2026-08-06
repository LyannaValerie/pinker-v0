#!/usr/bin/env bash
set -u
set -o pipefail

# Politica de codigo de saida.
#
# A contagem de falhas nunca e usada diretamente como status do processo: o
# shell trunca o codigo em modulo 256, de forma que 256 falhas produziriam
# zero, ou seja, sucesso aparente. Os codigos abaixo sao fixos e disjuntos.
readonly PINKER_FLAKE_EXIT_OK=0
readonly PINKER_FLAKE_EXIT_FAILURES=1
readonly PINKER_FLAKE_EXIT_USAGE=2
# Campanha concorrente ou lock em estado que nao autoriza prosseguir. Distinto
# de falha de teste: nenhum teste chegou a ser executado.
readonly PINKER_FLAKE_EXIT_LOCKED=3
# A identidade do controlador nao pode ser provada. Distinto de falha de teste e
# distinto de lock: o lote nao pode nem comecar a ser observado com seguranca.
readonly PINKER_FLAKE_EXIT_IDENTITY=4
readonly PINKER_FLAKE_EXIT_INTERRUPTED=130

# Nome do lock e do marker. O lock e um diretorio, adquirido por `mkdir`, que e
# atomico em POSIX. `flock` existe nesta maquina, mas nao e usado de
# proposito: um lock de `flock` vive preso a um descritor aberto e some
# junto com o processo, inclusive sob SIGKILL. O contrato aqui exige o
# oposto — o lock precisa sobreviver ao dono morto, carregando a
# identidade que permite a uma campanha posterior classifica-lo e
# recupera-lo.
readonly PINKER_FLAKE_LOCK_NAME='.lock'
readonly PINKER_FLAKE_MARKER_NAME='owner.marker'
readonly PINKER_FLAKE_MARKER_SCHEMA=1
# Ordem exata e fechada dos campos do marker. Qualquer desvio e invalido.
readonly PINKER_FLAKE_MARKER_FIELDS=(
    schema
    runner_pid
    runner_start_time
    mode
    head_git
    created_at_unix
    batch_id
)

pinker_flake_usage() {
    cat >&2 <<'USAGE'
uso: pinker-flake-runner.sh <mode> <runs> [threads] [filter]

  mode    identificador nao vazio do lote
  runs    inteiro decimal estritamente positivo, sem sinal e sem zero a esquerda
  threads "default" ou o valor repassado a --test-threads
  filter  nome exato de teste, ou @launcher-watchdog

ambiente:
  PINKER_FLAKE_RUN_TIMEOUT_SECONDS  quando definido, inteiro decimal
                                    estritamente positivo
  PINKER_FLAKE_TEST_BINARY          binario de teste ja compilado
USAGE
}

# Aceita somente inteiro decimal estritamente positivo.
#
# Rejeita ausente, vazio, texto, zero, negativo, sinal explicito, formato
# parcialmente numerico e espacos. Zeros a esquerda tambem sao rejeitados
# porque o Bash interpreta 010 como octal em contexto aritmetico, o que
# tornaria o valor aceito diferente do valor escrito.
#
# O limite de 18 digitos mantem o valor abaixo de 10^18, seguramente dentro
# do inteiro de 64 bits usado pelo Bash, de modo que a conversao aritmetica
# nunca satura nem muda de sinal.
pinker_flake_is_positive_int() {
    local value=${1-}
    [[ $value =~ ^[1-9][0-9]*$ ]] || return 1
    (( ${#value} <= 18 )) || return 1
    return 0
}

# Le o resumo do harness a partir do stdout da iteracao corrente.
#
# Emite "<executados> <passaram> <falharam>" e retorna 0 quando existe pelo
# menos uma linha de resumo reconhecida. Retorna 1 quando nenhuma linha e
# reconhecida, caso em que a iteracao nao pode ser considerada sucesso.
#
# Testes ignorados e filtrados nao contam como executados.
pinker_flake_parse_summary() {
    local file=${1-}
    local line total_passed=0 total_failed=0 found=0
    [[ -n $file && -r $file ]] || return 1
    while IFS= read -r line; do
        if [[ $line =~ ^test[[:space:]]result:[[:space:]].*[[:space:]]([0-9]+)[[:space:]]passed\;[[:space:]]([0-9]+)[[:space:]]failed ]]; then
            total_passed=$(( total_passed + BASH_REMATCH[1] ))
            total_failed=$(( total_failed + BASH_REMATCH[2] ))
            found=$(( found + 1 ))
        fi
    done < "$file"
    (( found > 0 )) || return 1
    printf '%s %s %s\n' "$(( total_passed + total_failed ))" "$total_passed" "$total_failed"
    return 0
}

# Traduz a contagem de falhas em codigo de saida fixo.
#
# Existe como funcao para que a politica seja provavel por regressao sem
# produzir centenas de falhas reais.
pinker_flake_exit_code_for() {
    local failures=${1-}
    if [[ ! $failures =~ ^[0-9]+$ ]]; then
        printf '%s\n' "$PINKER_FLAKE_EXIT_USAGE"
        return 0
    fi
    if (( failures == 0 )); then
        printf '%s\n' "$PINKER_FLAKE_EXIT_OK"
    else
        printf '%s\n' "$PINKER_FLAKE_EXIT_FAILURES"
    fi
    return 0
}

# Aceita `default` ou inteiro decimal estritamente positivo.
pinker_flake_is_valid_threads() {
    local value=${1-}
    [[ $value == default ]] && return 0
    pinker_flake_is_positive_int "$value"
}

# Rotulo seguro para compor nome de diretorio e campo de marker.
#
# Restringe a `[A-Za-z0-9._-]` porque `mode` e `threads` passam a compor o
# identificador do lote e o marker do lock. Um `mode` com quebra de linha
# corromperia o marker de forma indistinguivel de adulteracao, e o marker e
# autoridade para decidir se uma campanha alheia pode ser removida.
pinker_flake_is_safe_label() {
    local value=${1-}
    [[ $value =~ ^[A-Za-z0-9._-]+$ ]] || return 1
    (( ${#value} <= 64 )) || return 1
    return 0
}

# O identificador do lote compoe carimbo UTC com nanossegundos, mode, threads e
# PID, de modo que o limite util e maior que o de um rotulo isolado.
pinker_flake_is_safe_batch_id() {
    local value=${1-}
    [[ $value =~ ^[A-Za-z0-9._-]+$ ]] || return 1
    (( ${#value} >= 1 && ${#value} <= 160 )) || return 1
    return 0
}

# Le `starttime` (campo 22) de /proc/<pid>/stat.
#
# O corte usa o ultimo `) ` porque `comm` pode conter espaco e parentese.
# Emite o valor e retorna 0; retorna 1 quando o processo nao existe ou a linha
# nao pode ser interpretada.
pinker_flake_start_time_of() {
    local pid=${1-} stat rest
    [[ $pid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ -r "/proc/$pid/stat" ]] || return 1
    stat=$(<"/proc/$pid/stat") || return 1
    rest=${stat##*') '}
    [[ $rest != "$stat" ]] || return 1
    # shellcheck disable=SC2086
    set -- $rest
    [[ ${20-} =~ ^[0-9]+$ ]] || return 1
    printf '%s\n' "${20}"
}

# Extrai `state pgid sid start_time` do texto de uma linha de /proc/<pid>/stat.
#
# Mesmo corte pelo ultimo `) ` de `pinker_flake_start_time_of`, pela mesma
# razao: `comm` aceita espaco e parentese, e cortar pelo primeiro deslocaria
# todos os campos seguintes. Recebe o texto em vez do PID para que uma linha
# truncada, nao numerica ou ambigua possa ser provada por regressao sem
# depender de um processo real.
#
# Retorna 1 sem emitir nada quando qualquer campo exigido faltar ou nao for
# decimal. Identidade estruturalmente invalida nunca vira identidade parcial.
pinker_flake_stat_fields() {
    local texto=${1-} rest
    [[ -n $texto ]] || return 1
    rest=${texto##*') '}
    [[ $rest != "$texto" ]] || return 1
    # shellcheck disable=SC2086
    set -- $rest
    [[ ${1-} =~ ^[A-Za-z]$ ]] || return 1
    [[ ${3-} =~ ^[0-9]+$ ]] || return 1
    [[ ${4-} =~ ^[0-9]+$ ]] || return 1
    [[ ${20-} =~ ^[0-9]+$ ]] || return 1
    printf '%s %s %s %s\n' "${1}" "${3}" "${4}" "${20}"
}

# `state pgid sid start_time` de um PID vivo, ou 1 quando indisponivel.
pinker_flake_stat_fields_of() {
    local pid=${1-}
    [[ $pid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ -r "/proc/$pid/stat" ]] || return 1
    pinker_flake_stat_fields "$(<"/proc/$pid/stat")"
}

# Valida o anuncio de prontidao publicado pelo proprio controlador.
#
# Linha unica, marca fechada, versao fechada, quatro campos decimais e nada
# depois. Emite `pid start_time pgid sid` e retorna 0; retorna 1 sem emitir
# nada em qualquer outro caso. Falha fechada: um anuncio que nao passa aqui
# jamais vira identidade ativa.
pinker_flake_parse_identity_line() {
    local linha=${1-} marca versao pid start pgid sid extra
    read -r marca versao pid start pgid sid extra <<<"$linha"
    [[ $marca == pinker-flake-identity ]] || return 1
    [[ $versao == 1 ]] || return 1
    [[ -z ${extra:-} ]] || return 1
    [[ $pid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ $start =~ ^[0-9]+$ ]] || return 1
    [[ $pgid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ $sid =~ ^[1-9][0-9]*$ ]] || return 1
    printf '%s %s %s %s\n' "$pid" "$start" "$pgid" "$sid"
}

# Classifica a identidade do proprietario registrado no marker.
#
# Mesma autoridade usada pelo restante da contencao nativa:
#
#   live    PID existe e o start time confere: campanha em andamento;
#   missing /proc/<pid> comprovadamente nao existe: proprietario morreu;
#   reused  PID existe com start time diferente: outro processo herdou o numero;
#   unknown nao foi possivel provar identidade.
#
# `unknown` nunca autoriza remocao. Um lock so e recuperado sob `missing` ou
# `reused`, ambos provas positivas de que a campanha proprietaria acabou.
pinker_flake_classify_identity() {
    local pid=${1-} expected=${2-} observed
    if [[ ! $pid =~ ^[1-9][0-9]*$ || ! $expected =~ ^[0-9]+$ ]]; then
        printf 'unknown\n'
        return 0
    fi
    if [[ ! -e "/proc/$pid" ]]; then
        printf 'missing\n'
        return 0
    fi
    if ! observed=$(pinker_flake_start_time_of "$pid"); then
        printf 'unknown\n'
        return 0
    fi
    if [[ $observed == "$expected" ]]; then
        printf 'live\n'
    else
        printf 'reused\n'
    fi
}

# Valida o marker do lock de forma estrita.
#
# Exige exatamente os campos de `PINKER_FLAKE_MARKER_FIELDS`, nessa ordem, um
# por linha, no formato `chave: valor`. Linha faltando, linha sobrando, chave
# fora de ordem, chave repetida, valor vazio ou valor fora do dominio tornam o
# marker invalido. Campo repetido cai na verificacao de ordem, de modo que
# duplicata nunca passa despercebida.
#
# Emite `pid start_time mode head created_at batch_id` e retorna 0 quando
# valido. Retorna 1 sem emitir nada quando invalido.
pinker_flake_validate_marker() {
    local file=${1-}
    local -a linhas=()
    local linha indice esperado chave valor
    [[ -n $file && -f $file && ! -L $file && -r $file ]] || return 1
    while IFS= read -r linha || [[ -n $linha ]]; do
        linhas+=("$linha")
        (( ${#linhas[@]} <= ${#PINKER_FLAKE_MARKER_FIELDS[@]} + 1 )) || return 1
    done < "$file"
    (( ${#linhas[@]} == ${#PINKER_FLAKE_MARKER_FIELDS[@]} )) || return 1

    local pid= start= mode= head= created= batch=
    for indice in "${!PINKER_FLAKE_MARKER_FIELDS[@]}"; do
        esperado=${PINKER_FLAKE_MARKER_FIELDS[$indice]}
        linha=${linhas[$indice]}
        [[ $linha == "$esperado: "* ]] || return 1
        chave=$esperado
        valor=${linha#"$esperado: "}
        [[ -n $valor ]] || return 1
        case $chave in
            schema)
                [[ $valor == "$PINKER_FLAKE_MARKER_SCHEMA" ]] || return 1 ;;
            runner_pid)
                [[ $valor =~ ^[1-9][0-9]*$ ]] || return 1; pid=$valor ;;
            runner_start_time)
                [[ $valor =~ ^[0-9]+$ ]] || return 1; start=$valor ;;
            mode)
                pinker_flake_is_safe_label "$valor" || return 1; mode=$valor ;;
            head_git)
                [[ $valor =~ ^(unknown|[0-9a-f]{40})$ ]] || return 1; head=$valor ;;
            created_at_unix)
                [[ $valor =~ ^[0-9]+$ ]] || return 1; created=$valor ;;
            batch_id)
                pinker_flake_is_safe_batch_id "$valor" || return 1; batch=$valor ;;
            *)
                return 1 ;;
        esac
    done
    printf '%s %s %s %s %s %s\n' "$pid" "$start" "$mode" "$head" "$created" "$batch"
    return 0
}

# Serializa o marker na ordem canonica.
pinker_flake_marker_text() {
    printf 'schema: %s\n' "$PINKER_FLAKE_MARKER_SCHEMA"
    printf 'runner_pid: %s\n' "${1-}"
    printf 'runner_start_time: %s\n' "${2-}"
    printf 'mode: %s\n' "${3-}"
    printf 'head_git: %s\n' "${4-}"
    printf 'created_at_unix: %s\n' "${5-}"
    printf 'batch_id: %s\n' "${6-}"
}

# Modo biblioteca: permite que as regressoes carreguem as funcoes acima sem
# executar lote algum.
if [[ -n ${PINKER_FLAKE_LIB_ONLY:-} ]]; then
    return 0 2>/dev/null || exit 0
fi

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd -P)
evidence_root="$repo_root/target/pinker-flake-evidence"

# ---------------------------------------------------------------------------
# Validacao de uso.
#
# Ocorre antes de criar diretorio de evidencia, antes de apagar resumo
# anterior, antes de iniciar teste, antes de emitir progresso e antes de
# emitir resumo. Um erro de uso nao pode deixar o diretorio de evidencia em
# estado alterado nem remover o resultado de um lote anterior.
# ---------------------------------------------------------------------------
mode=${1-}
runs=${2-}
threads=${3:-default}
filter=${4:-}

if [[ -z $mode ]]; then
    printf 'pinker-flake-runner: mode ausente\n' >&2
    pinker_flake_usage
    exit "$PINKER_FLAKE_EXIT_USAGE"
fi

if ! pinker_flake_is_safe_label "$mode"; then
    printf 'pinker-flake-runner: mode invalido: %s\n' "$mode" >&2
    printf 'pinker-flake-runner: exige [A-Za-z0-9._-] e ate 64 caracteres\n' >&2
    pinker_flake_usage
    exit "$PINKER_FLAKE_EXIT_USAGE"
fi

if ! pinker_flake_is_positive_int "$runs"; then
    printf 'pinker-flake-runner: runs invalido: %s\n' "${runs-<ausente>}" >&2
    printf 'pinker-flake-runner: exige inteiro decimal estritamente positivo\n' >&2
    pinker_flake_usage
    exit "$PINKER_FLAKE_EXIT_USAGE"
fi

if [[ -n ${PINKER_FLAKE_RUN_TIMEOUT_SECONDS+definido} ]]; then
    if ! pinker_flake_is_positive_int "${PINKER_FLAKE_RUN_TIMEOUT_SECONDS}"; then
        printf 'pinker-flake-runner: PINKER_FLAKE_RUN_TIMEOUT_SECONDS invalido: %s\n' \
            "${PINKER_FLAKE_RUN_TIMEOUT_SECONDS}" >&2
        pinker_flake_usage
        exit "$PINKER_FLAKE_EXIT_USAGE"
    fi
    per_run_timeout=${PINKER_FLAKE_RUN_TIMEOUT_SECONDS}
else
    per_run_timeout=300
fi

if ! pinker_flake_is_valid_threads "$threads"; then
    printf 'pinker-flake-runner: threads invalido: %s\n' "$threads" >&2
    pinker_flake_usage
    exit "$PINKER_FLAKE_EXIT_USAGE"
fi

test_binary=${PINKER_FLAKE_TEST_BINARY:-}

# ---------------------------------------------------------------------------
# Exclusividade por checkout.
#
# Duas campanhas sobre o mesmo `target` compartilhavam progresso, resumo,
# sandboxes, markers e arquivos auxiliares. O runner apagava
# `PROGRESS-<mode>.txt` e `SUMMARY-<mode>.txt` no inicio de cada lote, de modo
# que a segunda campanha destruia a evidencia da primeira e podia terminar
# verde escondendo as iteracoes falhadas da outra. Isso ocorreu de fato durante
# a correcao da PR 422.
#
# Proibicao operacional nao basta. A autoridade passa a impedir tecnicamente a
# concorrencia: um unico lock por checkout, independente de mode, filtro ou
# threads.
# ---------------------------------------------------------------------------
lock_dir="$evidence_root/$PINKER_FLAKE_LOCK_NAME"
lock_marker="$lock_dir/$PINKER_FLAKE_MARKER_NAME"
lock_owned=
runner_start=$(pinker_flake_start_time_of $$ || printf 'unknown')
# Grupo e sessao do proprio runner. Nenhum sinal coletivo pode alcanca-los: se o
# `setsid` da iteracao falhasse em silencio, o controlador nasceria neste grupo
# e conter a arvore alcancaria o processo de testes que iniciou a campanha.
runner_pgid=unknown
runner_sid=unknown
if runner_fields=$(pinker_flake_stat_fields_of $$); then
    read -r _runner_state runner_pgid runner_sid _runner_start <<<"$runner_fields"
fi
head_sha=$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf 'unknown')
[[ $head_sha =~ ^[0-9a-f]{40}$ ]] || head_sha=unknown
created_at=$(date +%s)
batch_id="$(date -u +%Y%m%dT%H%M%S.%NZ)-${mode}-${threads}-$$"

if [[ $runner_start == unknown ]]; then
    # Sem identidade propria comprovavel nao ha como registrar quem detem o
    # lock, e portanto nao ha como uma campanha posterior classificar este
    # lock. Falha fechada em vez de gravar um marker que ja nasce invalido.
    printf 'pinker-flake-runner: start time do proprio runner indisponivel\n' >&2
    exit "$PINKER_FLAKE_EXIT_LOCKED"
fi

# Gancho de teste, exercido apenas quando explicitamente configurado. Mesma
# forma usada por `scripts/pinker-cleanup.sh` para provar janelas de corrida
# sem depender de temporizacao real.
pinker_flake_test_hook() {
    local stage=${1-}
    [[ -n ${PINKER_FLAKE_TEST_HOOK:-} ]] || return 0
    "$PINKER_FLAKE_TEST_HOOK" "$stage" "$lock_dir" || return 1
    return 0
}

pinker_flake_write_marker() {
    local destino=$1 temporario="$lock_dir/.marker.$$"
    pinker_flake_marker_text \
        "$$" "$runner_start" "$mode" "$head_sha" "$created_at" "$batch_id" \
        > "$temporario" || return 1
    mv -f -- "$temporario" "$destino" || return 1
    return 0
}

# Remove um lock cuja identidade ja foi provada obsoleta.
#
# Nao move o diretorio: remove o marker validado e depois `rmdir`. `rmdir` falha
# quando alguem acrescentou conteudo, e `mkdir` de uma campanha nova so pode
# vencer depois que este `rmdir` completar. Assim duas recuperacoes simultaneas
# convergem — uma cria o lock, a outra reencontra um lock vivo e e rejeitada —
# sem que nenhuma delas remova o lock de uma campanha em andamento.
pinker_flake_recover_stale_lock() {
    local esperado=$1 atual
    pinker_flake_test_hook before-lock-removal || return 1
    # Revalida imediatamente antes de remover: se a identidade registrada mudou
    # entre a classificacao e a remocao, o lock deixa de ser o mesmo objeto e
    # deve ser preservado.
    atual=$(pinker_flake_validate_marker "$lock_marker") || return 1
    [[ $atual == "$esperado" ]] || return 1
    rm -f -- "$lock_marker" || return 1
    rmdir -- "$lock_dir" 2>/dev/null || return 1
    return 0
}

pinker_flake_acquire_lock() {
    local campos identidade pid start
    mkdir -p "$evidence_root" || return "$PINKER_FLAKE_EXIT_LOCKED"

    if mkdir "$lock_dir" 2>/dev/null; then
        if ! pinker_flake_write_marker "$lock_marker"; then
            rm -f -- "$lock_marker"
            rmdir -- "$lock_dir" 2>/dev/null || true
            printf 'pinker-flake-runner: falha ao registrar o marker do lock\n' >&2
            return "$PINKER_FLAKE_EXIT_LOCKED"
        fi
        if ! pinker_flake_validate_marker "$lock_marker" > /dev/null; then
            rm -f -- "$lock_marker"
            rmdir -- "$lock_dir" 2>/dev/null || true
            printf 'pinker-flake-runner: marker do lock nao passou na releitura\n' >&2
            return "$PINKER_FLAKE_EXIT_LOCKED"
        fi
        lock_owned=1
        return 0
    fi

    # O lock existe. Nenhuma decisao pode vir do nome.
    if [[ -L $lock_dir ]]; then
        printf 'pinker-flake-runner: lock e symlink, recusado: %s\n' "$lock_dir" >&2
        return "$PINKER_FLAKE_EXIT_LOCKED"
    fi
    if [[ ! -d $lock_dir ]]; then
        printf 'pinker-flake-runner: lock existe e nao e diretorio: %s\n' "$lock_dir" >&2
        return "$PINKER_FLAKE_EXIT_LOCKED"
    fi
    if [[ -L $lock_marker ]]; then
        printf 'pinker-flake-runner: marker do lock e symlink, recusado\n' >&2
        return "$PINKER_FLAKE_EXIT_LOCKED"
    fi
    if ! campos=$(pinker_flake_validate_marker "$lock_marker"); then
        printf 'pinker-flake-runner: marker do lock ausente ou invalido, falha fechada\n' >&2
        printf 'pinker-flake-runner: preservado para inspecao: %s\n' "$lock_dir" >&2
        return "$PINKER_FLAKE_EXIT_LOCKED"
    fi
    read -r pid start _ <<<"$campos"
    identidade=$(pinker_flake_classify_identity "$pid" "$start")
    case $identidade in
        live)
            printf 'pinker-flake-runner: campanha concorrente no mesmo checkout\n' >&2
            printf 'pinker-flake-runner: owner_pid=%s owner_start_time=%s identity=live\n' \
                "$pid" "$start" >&2
            printf 'pinker-flake-runner: lock=%s\n' "$lock_dir" >&2
            printf 'pinker-flake-runner: nenhuma evidencia foi tocada\n' >&2
            return "$PINKER_FLAKE_EXIT_LOCKED"
            ;;
        missing|reused)
            if ! pinker_flake_recover_stale_lock "$campos"; then
                printf 'pinker-flake-runner: lock obsoleto nao pode ser recuperado com seguranca\n' >&2
                printf 'pinker-flake-runner: preservado: %s\n' "$lock_dir" >&2
                return "$PINKER_FLAKE_EXIT_LOCKED"
            fi
            # Uma unica tentativa, para concluir a transacao de recuperacao.
            # Nao e espera nem retry: se outra campanha venceu a corrida, ela
            # detem o lock e esta instancia e rejeitada como concorrente.
            if mkdir "$lock_dir" 2>/dev/null; then
                if ! pinker_flake_write_marker "$lock_marker" ||
                   ! pinker_flake_validate_marker "$lock_marker" > /dev/null; then
                    rm -f -- "$lock_marker"
                    rmdir -- "$lock_dir" 2>/dev/null || true
                    printf 'pinker-flake-runner: falha ao registrar o marker apos recuperacao\n' >&2
                    return "$PINKER_FLAKE_EXIT_LOCKED"
                fi
                lock_owned=1
                printf 'pinker-flake-runner: lock obsoleto recuperado (identity=%s owner_pid=%s)\n' \
                    "$identidade" "$pid" >&2
                return 0
            fi
            printf 'pinker-flake-runner: outra campanha adquiriu o lock durante a recuperacao\n' >&2
            return "$PINKER_FLAKE_EXIT_LOCKED"
            ;;
        *)
            printf 'pinker-flake-runner: identidade do proprietario desconhecida, falha fechada\n' >&2
            printf 'pinker-flake-runner: owner_pid=%s owner_start_time=%s identity=unknown\n' \
                "$pid" "$start" >&2
            printf 'pinker-flake-runner: preservado: %s\n' "$lock_dir" >&2
            return "$PINKER_FLAKE_EXIT_LOCKED"
            ;;
    esac
}

# Libera somente o lock desta instancia.
#
# Revalida diretorio e marker antes de remover. Identidade divergente significa
# que o lock ja nao pertence a esta campanha, e nesse caso ele e preservado.
pinker_flake_release_lock() {
    local campos pid start batch
    [[ -n $lock_owned ]] || return 0
    lock_owned=
    [[ -L $lock_dir ]] && return 0
    [[ -d $lock_dir ]] || return 0
    campos=$(pinker_flake_validate_marker "$lock_marker") || return 0
    read -r pid start _ _ _ batch <<<"$campos"
    [[ $pid == "$$" && $start == "$runner_start" && $batch == "$batch_id" ]] || return 0
    rm -f -- "$lock_marker" || return 0
    rmdir -- "$lock_dir" 2>/dev/null || return 0
    return 0
}

if ! pinker_flake_acquire_lock; then
    exit "$PINKER_FLAKE_EXIT_LOCKED"
fi

# ---------------------------------------------------------------------------
# Namespace exclusivo do lote.
#
# A autoridade do lote e o proprio diretorio, jamais um caminho compartilhado
# por mode. `PROGRESS-<mode>.txt` e `SUMMARY-<mode>.txt` passam a ser projecao
# do ultimo lote concluido, publicada por rename atomico somente no fim, e
# nunca removida no inicio.
# ---------------------------------------------------------------------------
batches_root="$evidence_root/batches"
batch_dir="$batches_root/$batch_id"
mkdir -p "$batch_dir"
progress_file="$batch_dir/PROGRESS.txt"
summary_file="$batch_dir/SUMMARY.txt"
manifest_file="$batch_dir/MANIFEST.txt"
legacy_progress="$evidence_root/PROGRESS-$mode.txt"
legacy_summary="$evidence_root/SUMMARY-$mode.txt"

{
    printf 'schema=1\n'
    printf 'batch_id=%s\n' "$batch_id"
    printf 'head_sha=%s\n' "$head_sha"
    printf 'mode=%s\n' "$mode"
    printf 'runs=%s\n' "$runs"
    printf 'threads=%s\n' "$threads"
    printf 'test_filter=%s\n' "${filter:-<complete-file>}"
    printf 'compiled_test_binary=%s\n' "${test_binary:-false}"
    printf 'run_timeout_seconds=%s\n' "$per_run_timeout"
    printf 'runner_pid=%s\n' "$$"
    printf 'runner_start_time=%s\n' "$runner_start"
    printf 'created_at_unix=%s\n' "$created_at"
    printf 'batch_dir=%s\n' "$batch_dir"
} > "$manifest_file"

# Publica a projecao legada por temporario e rename atomico.
#
# Um leitor nunca observa arquivo parcial, e um lote interrompido jamais
# substitui o ultimo resumo completo: a publicacao so ocorre no fim do lote.
pinker_flake_publish_legacy() {
    local origem=$1 destino=$2 temporario
    [[ -f $origem ]] || return 0
    temporario="$destino.$$.parcial"
    {
        cat -- "$origem"
        printf 'authority=%s\n' "$batch_dir"
        printf 'projection=last-completed-batch\n'
    } > "$temporario" || { rm -f -- "$temporario"; return 1; }
    mv -f -- "$temporario" "$destino" || { rm -f -- "$temporario"; return 1; }
    return 0
}

proc_start_time() {
    local pid=$1 stat rest
    [[ -r "/proc/$pid/stat" ]] || return 1
    stat=$(<"/proc/$pid/stat") || return 1
    rest=${stat##*) }
    set -- $rest
    printf '%s\n' "${20:-unknown}"
}

# ---------------------------------------------------------------------------
# Ciclo de vida do controlador da iteracao.
#
# Estados explicitos, jamais inferidos da combinacao de variaveis vazias:
#
#   idle      nenhum controlador iniciado nesta iteracao;
#   starting  criacao iniciada, identidade ainda nao confirmada;
#   active    PID, start time, PGID e SID capturados e validados;
#   reaping   encerramento e wait em andamento;
#   finished  filho aguardado e estado limpo.
#
# A fase `starting` existe porque os traps de INT, TERM e HUP ja estao ativos
# quando o controlador nasce. Antes desta correcao o handler chamava direto o
# cleanup, cuja validacao exigia `active_start` — capturado somente depois do
# `setsid`. Um sinal nessa janela encontrava a validacao falhando, o cleanup
# retornava sem encerrar nada, e o runner saia com 130 deixando `timeout`,
# harness e descendentes vivos: uma interrupcao aparentemente correta com
# residuo. A janela foi descoberta durante a PR 424 e deliberadamente deixada
# fora do escopo daquela unidade.
#
# A eliminacao e por construcao, nao por temporizacao: durante `starting` o
# sinal e registrado como interrupcao pendente e processado somente depois de a
# identidade estar completa.
# ---------------------------------------------------------------------------
active_state=idle
active_pid=
active_start=
active_pgid=
active_sid=
monitor_pid=
identity_channel=
identity_fd=
pending_signal=
pending_state=
pending_controller=no
interrupt_in_progress=
interrupt_residual_group=unknown

# Orcamento do anuncio de prontidao. Generoso para uma maquina saturada e
# finito por principio: espera infinita nao e contencao.
readonly PINKER_FLAKE_IDENTITY_TIMEOUT_SECONDS=30
# Prazo de cada fase do encerramento: 250 x 20 ms = 5 s.
readonly PINKER_FLAKE_REAP_ATTEMPTS=250
readonly PINKER_FLAKE_REAP_INTERVAL=0.02

# Preambulo executado pelo proprio controlador antes do harness.
#
# `setsid` nao forka nesta maquina — o filho de background de um shell sem
# controle de job nao lidera grupo, e util-linux so forka quando ja lidera —, de
# modo que este `bash` mantem o PID de `$!` e o `exec` final preserva PID, start
# time, PGID e SID. A igualdade `pid = pgid = sid` nao e presumida: ela e
# medida aqui, anunciada, e reexigida pelo validador.
#
# O anuncio acontece antes do `exec`, portanto antes do harness. Uma identidade
# publicada pelo proprio processo nao corre com a morte dele: `/proc` some
# quando o Bash colhe o filho de background, e uma captura que dependesse de
# `/proc` perderia a identidade de um harness rapido.
readonly PINKER_FLAKE_CONTROLLER_PREAMBLE='
canal=$PINKER_FLAKE_IDENTITY_CHANNEL
stat=$(</proc/self/stat) || exit 97
rest=${stat##*") "}
[[ $rest != "$stat" ]] || exit 97
campos=($rest)
printf "pinker-flake-identity 1 %s %s %s %s\n" \
    "$$" "${campos[19]}" "${campos[2]}" "${campos[3]}" > "$canal" || exit 97
exec "$@"
'

# Gancho estritamente de teste para congelar pontos da inicializacao.
#
# Separado de `PINKER_FLAKE_TEST_HOOK` de proposito: ampliar o gancho antigo
# faria um hook ja existente receber estagios que ele nunca esperou, e o unico
# consumidor atual reescreve o marker do lock em qualquer estagio que receba.
# Exige duas variaveis — o programa e a lista explicita de estagios — para que
# nem uma configuracao parcial ative comportamento. Ausente, producao nao muda
# em nada.
pinker_flake_startup_hook() {
    local stage=${1-}
    [[ -n ${PINKER_FLAKE_STARTUP_HOOK:-} ]] || return 0
    case " ${PINKER_FLAKE_STARTUP_HOOK_STAGES:-} " in
        *" $stage "*) ;;
        *) return 0 ;;
    esac
    "$PINKER_FLAKE_STARTUP_HOOK" "$stage" "$batch_dir" "${iteration:-0}" || true
    return 0
}

# Canal exclusivo da iteracao para o anuncio de prontidao.
#
# Aberto em leitura-escrita: `open` de FIFO nunca bloqueia nesse modo e o canal
# nunca sinaliza EOF, de modo que a espera termina por dado ou por prazo, e
# nunca por fechamento acidental do escritor. O canal nao sobrevive a iteracao.
pinker_flake_open_identity_channel() {
    identity_channel="$batch_dir/.identity-${iteration:-0}-$$.fifo"
    rm -f -- "$identity_channel" || return 1
    mkfifo -m 0600 -- "$identity_channel" 2>/dev/null || return 1
    exec {identity_fd}<>"$identity_channel" || return 1
    return 0
}

pinker_flake_close_identity_channel() {
    if [[ -n $identity_fd ]]; then
        exec {identity_fd}>&- 2>/dev/null || true
        identity_fd=
    fi
    if [[ -n $identity_channel ]]; then
        rm -f -- "$identity_channel" 2>/dev/null || true
        identity_channel=
    fi
    return 0
}

# Espera o anuncio, com orcamento finito.
#
# `read` devolve assim que o dado chega: a espera e por evento, e o prazo de um
# segundo existe apenas para reavaliar o orcamento quando um sinal tratado
# interrompe a leitura. O sinal ja ficou registrado como interrupcao pendente e
# sera processado assim que a identidade estiver completa — abortar aqui
# devolveria justamente a identidade incompleta que esta correcao elimina.
pinker_flake_await_identity() {
    local restante=$PINKER_FLAKE_IDENTITY_TIMEOUT_SECONDS linha campos
    [[ -n $identity_fd ]] || return 1
    while (( restante > 0 )); do
        if IFS= read -r -t 1 linha <&"$identity_fd"; then
            campos=$(pinker_flake_parse_identity_line "$linha") || return 1
            printf '%s\n' "$campos"
            return 0
        fi
        restante=$(( restante - 1 ))
    done
    return 1
}

# Captura e valida a identidade completa do controlador.
#
# Sucesso significa: anuncio integro, pertencente ao filho direto desta
# iteracao, com o controlador liderando grupo e sessao proprios, fora do grupo e
# da sessao do runner, e concordante com `/proc` enquanto o processo existir.
# Qualquer divergencia falha fechada.
pinker_flake_capture_identity() {
    local campos pid start pgid sid observados o_state o_pgid o_sid o_start
    campos=$(pinker_flake_await_identity) || return 1
    read -r pid start pgid sid <<<"$campos"
    [[ $pid == "${controller_pid:-}" ]] || return 1
    [[ $pgid == "$pid" && $sid == "$pid" ]] || return 1
    [[ $runner_pgid != unknown && $runner_sid != unknown ]] || return 1
    [[ $pgid != "$runner_pgid" && $sid != "$runner_sid" ]] || return 1
    if observados=$(pinker_flake_stat_fields_of "$pid"); then
        read -r o_state o_pgid o_sid o_start <<<"$observados"
        [[ $o_start == "$start" ]] || return 1
        [[ $o_pgid == "$pgid" ]] || return 1
        [[ $o_sid == "$sid" ]] || return 1
    fi
    active_pid=$pid
    active_start=$start
    active_pgid=$pgid
    active_sid=$sid
    return 0
}

# A identidade ativa esta completa e estruturalmente valida?
#
# Ponto unico do contrato: `active` exige os quatro campos, e nao a combinacao
# implicita de variaveis vazias que o runner usava antes.
pinker_flake_identity_complete() {
    [[ $active_pid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ $active_start =~ ^[0-9]+$ ]] || return 1
    [[ $active_pgid =~ ^[1-9][0-9]*$ ]] || return 1
    [[ $active_sid =~ ^[1-9][0-9]*$ ]] || return 1
    return 0
}

# Revalida a identidade ativa contra `/proc`, imediatamente antes de agir.
#
#   live     os quatro campos conferem e o processo executa;
#   zombie   terminou e ainda nao foi colhido: nada a sinalizar;
#   gone     /proc/<pid> nao existe: prova positiva de ausencia;
#   unknown  incompleta, ilegivel ou divergente. Nunca autoriza sinal.
pinker_flake_active_identity_state() {
    local observados o_state o_pgid o_sid o_start
    if ! pinker_flake_identity_complete; then
        printf 'unknown\n'
        return 0
    fi
    if [[ ! -e "/proc/$active_pid" ]]; then
        printf 'gone\n'
        return 0
    fi
    if ! observados=$(pinker_flake_stat_fields_of "$active_pid"); then
        printf 'unknown\n'
        return 0
    fi
    read -r o_state o_pgid o_sid o_start <<<"$observados"
    if [[ $o_start != "$active_start" || $o_pgid != "$active_pgid" || $o_sid != "$active_sid" ]]; then
        printf 'unknown\n'
        return 0
    fi
    if [[ $o_state == Z ]]; then
        printf 'zombie\n'
    else
        printf 'live\n'
    fi
}

# Contem o filho direto pelo PID.
#
# Contencao primaria: `timeout` propaga o sinal ao comando que administra, de
# modo que alcancar o filho direto derruba a maior parte da arvore. Quando a
# identidade esta completa, o sinal exige revalidacao — um PID ja colhido pode
# ter sido reutilizado. Quando ainda nao ha identidade, o filho e
# comprovadamente nao aguardado e o numero nao pode nomear outra coisa.
pinker_flake_signal_direct_child() {
    local sinal=$1 estado
    [[ -n $active_pid ]] || return 0
    if pinker_flake_identity_complete; then
        estado=$(pinker_flake_active_identity_state)
        [[ $estado == live || $estado == zombie ]] || return 0
    fi
    kill -"$sinal" -- "$active_pid" 2>/dev/null || true
    return 0
}

# Sinal coletivo, autorizado somente por identidade revalidada.
#
# Identidade desconhecida nao autoriza sinalizacao coletiva: um PGID pode ter
# desaparecido e passado a nomear outro grupo entre a captura e o sinal.
pinker_flake_signal_group() {
    local sinal=$1
    [[ $(pinker_flake_active_identity_state) == live ]] || return 1
    kill -"$sinal" -- "-$active_pgid" 2>/dev/null || true
    return 0
}

# Quantos processos vivos, fora o controlador, ainda pertencem ao grupo.
#
# A varredura so vale enquanto o controlador nao foi colhido: ele lidera o
# grupo, e o numero do grupo nao pode nomear outra coisa enquanto o seu PID
# permanecer preso. Depois do `wait` essa garantia acaba, e por isso a
# confirmacao acontece antes dele. Zumbis nao contam: nao executam nada.
pinker_flake_group_survivors() {
    local entrada pid texto rest total=0
    if ! pinker_flake_identity_complete; then
        printf 'unknown\n'
        return 0
    fi
    for entrada in /proc/[0-9]*/stat; do
        pid=${entrada#/proc/}
        pid=${pid%/stat}
        [[ $pid == "$active_pid" ]] && continue
        [[ -r $entrada ]] || continue
        texto=$(<"$entrada") || continue
        rest=${texto##*') '}
        [[ $rest != "$texto" ]] || continue
        # shellcheck disable=SC2086
        set -- $rest
        [[ ${1-} == Z ]] && continue
        [[ ${3-} == "$active_pgid" ]] && total=$(( total + 1 ))
    done
    printf '%s\n' "$total"
    return 0
}

# A arvore da iteracao esta silenciosa?
#
# Sem identidade completa nao ha grupo autorizado a observar, e o silencio se
# reduz ao unico fato provavel: o filho direto, cujo numero nao pode nomear
# outra coisa enquanto o runner nao o colher.
pinker_flake_tree_quiet() {
    if ! pinker_flake_identity_complete; then
        [[ -n $active_pid ]] || return 0
        kill -0 "$active_pid" 2>/dev/null && return 1
        return 0
    fi
    [[ $(pinker_flake_active_identity_state) != live ]] || return 1
    [[ $(pinker_flake_group_survivors) == 0 ]] || return 1
    return 0
}

pinker_flake_await_tree_quiescence() {
    local _tentativa
    for (( _tentativa = 0; _tentativa < PINKER_FLAKE_REAP_ATTEMPTS; _tentativa++ )); do
        pinker_flake_tree_quiet && return 0
        sleep "$PINKER_FLAKE_REAP_INTERVAL"
    done
    return 1
}

# Encerra e aguarda a arvore do controlador, em ordem explicita.
#
# TERM na arvore autorizada, prazo limitado, revalidacao, KILL somente nos
# sobreviventes autorizados, e por fim o `wait` do filho direto — sem o qual o
# controlador vira zumbi dentro do runner, que e residuo tanto quanto um
# processo vivo.
pinker_flake_reap_active_tree() {
    [[ -n $active_pid ]] || return 0
    active_state=reaping
    pinker_flake_signal_direct_child TERM
    pinker_flake_signal_group TERM || true
    pinker_flake_await_tree_quiescence || true
    if ! pinker_flake_tree_quiet; then
        pinker_flake_signal_direct_child KILL
        pinker_flake_signal_group KILL || true
        pinker_flake_await_tree_quiescence || true
    fi
    interrupt_residual_group=$(pinker_flake_group_survivors)
    wait "$active_pid" 2>/dev/null || true
    active_pid=
    active_start=
    active_pgid=
    active_sid=
    return 0
}

# Encerra ou aguarda o subshell monitor.
#
# O monitor pertence ao runner tanto quanto o controlador. Ele sai sozinho ao
# observar a morte do controlador — que so e observavel depois do `wait` do
# filho direto —, e a espera aqui e o que impede que ele seja reparentado para o
# init ou que escreva na evidencia depois de ela ter sido movida. A contencao
# por PID e limitada e so age quando a saida espontanea nao acontece: derrubar o
# subshell no meio de uma amostragem deixaria os processos que ele acabou de
# forkar orfaos.
pinker_flake_reap_monitor() {
    local _tentativa
    [[ -n $monitor_pid ]] || return 0
    for (( _tentativa = 0; _tentativa < PINKER_FLAKE_REAP_ATTEMPTS; _tentativa++ )); do
        kill -0 "$monitor_pid" 2>/dev/null || break
        sleep "$PINKER_FLAKE_REAP_INTERVAL"
    done
    if kill -0 "$monitor_pid" 2>/dev/null; then
        kill -TERM -- "$monitor_pid" 2>/dev/null || true
    fi
    wait "$monitor_pid" 2>/dev/null || true
    monitor_pid=
    return 0
}

cleanup_active() {
    pinker_flake_reap_active_tree
    pinker_flake_reap_monitor
    pinker_flake_close_identity_channel
    active_state=finished
    return 0
}

# Conclui uma interrupcao ja congelada. Nunca retorna.
pinker_flake_finish_interrupted() {
    local interrupted_name interrupted_dir stopped_at interrupted_duration
    cleanup_active
    if [[ -n "${tmp:-}" && -d "${tmp:-}" ]]; then
        interrupted_name=${tmp##*/}
        interrupted_name=${interrupted_name#.running-}
        interrupted_dir="$batch_dir/INTERRUPTED-$interrupted_name"
        stopped_at=$(date +%s%3N)
        if [[ "${start:-}" =~ ^[0-9]+$ ]]; then
            interrupted_duration=$((stopped_at-start))
        else
            interrupted_duration=0
        fi
        mv -- "$tmp" "$interrupted_dir"
        preserve_failure \
            "$interrupted_dir" \
            "${iteration:-unknown}" \
            "$interrupted_duration" \
            "$PINKER_FLAKE_EXIT_INTERRUPTED" \
            "${controller_pid:-0}" \
            interrupted
        # Relatorio humano da primeira causa. O schema publico do manifesto nao
        # muda de forma: continuam sendo linhas `chave=valor`.
        {
            printf 'interrupt_signal=%s\n' "${pending_signal:-unknown}"
            printf 'interrupt_state=%s\n' "${pending_state:-unknown}"
            printf 'interrupt_controller_existed=%s\n' "$pending_controller"
            printf 'interrupt_residual_group=%s\n' "${interrupt_residual_group:-unknown}"
        } >> "$interrupted_dir/manifest.txt"
    fi
    exit "$PINKER_FLAKE_EXIT_INTERRUPTED"
}

# Handler unico de INT, TERM e HUP.
#
# Durante `starting` a interrupcao e apenas registrada: o handler nao executa
# cleanup com identidade incompleta, nao sai do runner e nao ignora o sinal em
# definitivo. A primeira causa vence e nunca e substituida, e um segundo sinal
# durante o encerramento nao reinicia a escalada.
interrupt_runner() {
    local sinal=${1:-UNKNOWN}
    if [[ -z $pending_signal ]]; then
        pending_signal=$sinal
        pending_state=$active_state
        pending_controller=no
        [[ -n $active_pid ]] && pending_controller=yes
    fi
    if [[ $active_state == starting ]]; then
        return 0
    fi
    [[ -z $interrupt_in_progress ]] || return 0
    interrupt_in_progress=1
    pinker_flake_finish_interrupted
}

# Processa uma interrupcao registrada durante `starting`. Nunca retorna quando
# ha causa pendente.
pinker_flake_settle_pending() {
    [[ -n $pending_signal ]] || return 0
    [[ -z $interrupt_in_progress ]] || return 0
    interrupt_in_progress=1
    pinker_flake_finish_interrupted
}

# Falha fechada quando a identidade do controlador nao pode ser provada.
#
# Nao inicia o monitor, nao executa os testes seguintes, nao publica PASS e nao
# publica resumo. Contem o filho direto, aguarda-o, preserva evidencia
# diagnostica e termina com codigo nao zero estavel.
pinker_flake_fail_identity() {
    local destino
    active_state=reaping
    pinker_flake_settle_pending
    interrupt_in_progress=1
    cleanup_active
    if [[ -n "${tmp:-}" && -d "${tmp:-}" ]]; then
        destino="$batch_dir/IDENTITY-FAILURE-${tmp##*/.running-}"
        mv -- "$tmp" "$destino"
        preserve_failure \
            "$destino" \
            "${iteration:-unknown}" \
            0 \
            "$PINKER_FLAKE_EXIT_IDENTITY" \
            "${controller_pid:-0}" \
            identity-capture-failed
    fi
    exit "$PINKER_FLAKE_EXIT_IDENTITY"
}

# A liberacao do lock fica no EXIT porque a interrupcao termina por `exit`:
# sucesso, falha comum, erro apos a aquisicao, SIGINT, SIGTERM e SIGHUP passam
# todos por aqui. SIGKILL nao executa trap algum, e por isso o lock registra
# identidade suficiente para que uma campanha posterior o classifique e o
# recupere.
trap 'cleanup_active; pinker_flake_release_lock' EXIT
trap 'interrupt_runner INT' INT
trap 'interrupt_runner TERM' TERM
trap 'interrupt_runner HUP' HUP

preserve_failure() {
    local run_dir=$1 iteration=$2 duration=$3 exit_code=$4 controller_pid=$5
    local reason=${6:-unspecified}
    local failed_tests pids pid
    mkdir -p "$run_dir/proc"
    failed_tests=$(sed -n 's/^---- \(.*\) stdout ----$/\1/p; s/^    \([^ ]*\)\r\{0,1\}$/\1/p' "$run_dir/stdout" | sort -u)
    {
        printf 'iteration=%s\nmode=%s\ntest_filter=%s\ntest_threads=%s\n' "$iteration" "$mode" "${filter:-<complete-file>}" "$threads"
        printf 'reason=%s\n' "$reason"
        printf 'runner_pid=%s\nrunner_start_time=%s\nharness_pid=%s\nharness_start_time=%s\n' "$$" "$(proc_start_time $$ 2>/dev/null || printf unknown)" "$controller_pid" "$(proc_start_time "$controller_pid" 2>/dev/null || printf exited)"
        printf 'duration_ms=%s\nexit_code=%s\nhead_git=%s\n' "$duration" "$exit_code" "$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf unknown)"
        printf 'compiled_test_binary=%s\n' "${test_binary:-false}"
        printf 'tests_executed=%s\n' "${run_executed:-unknown}"
        printf 'summary_recognized=%s\n' "${run_summary_recognized:-unknown}"
        printf 'failed_tests=%s\n' "${failed_tests:-unparsed-see-stdout-stderr}"
    } > "$run_dir/manifest.txt"
    ps -eo pid,ppid,pgid,sid,stat,lstart,comm,args > "$run_dir/processes.txt" 2>&1 || true
    find "$repo_root/target/pinker-exec" -xdev -maxdepth 6 -printf '%y %p -> %l\n' > "$run_dir/sandbox-tree.txt" 2>&1 || true
    find "$repo_root/target" -maxdepth 3 -type f \( -name 'owner.marker' -o -name '*.pid' -o -name 'supervisor-result-*' -o -name '*lifecycle*' -o -name '*result*' \) -print0 2>/dev/null |
        while IFS= read -r -d '' file; do
            dest="$run_dir/auxiliary/${file#"$repo_root/"}"
            mkdir -p "$(dirname "$dest")"
            cp -a -- "$file" "$dest" 2>/dev/null || true
        done
    find "$run_dir/auxiliary" -type f -name '*lifecycle*' -exec sed -n '1,$p' {} + \
        > "$run_dir/lifecycle-events.txt" 2>/dev/null || true
    grep -rhoE '^(primary|secondary|tree_shutdown_proven|sandbox_disposition|status_|schema)=' \
        "$run_dir/auxiliary" > "$run_dir/structured-results.txt" 2>/dev/null || true
    grep -hE 'watchdog_ready|launcher_ready|guest_started|primary_reason_latched|secondary_failure|result_published' \
        "$run_dir/lifecycle-events.txt" >> "$run_dir/manifest.txt" 2>/dev/null || true
    for field in owner_pid owner_start_time launcher_pid launcher_start_time guest_pid process_group_id watchdog_pid state; do
        value=$(grep -rho "^$field: .*" "$run_dir/auxiliary" 2>/dev/null | head -n 1 | cut -d' ' -f2-)
        [[ -n "$value" ]] && printf '%s=%s\n' "$field" "$value" >> "$run_dir/manifest.txt"
    done
    pids=$(grep -rhoE '(^|[^[:digit:]])[1-9][0-9]{1,8}([^[:digit:]]|$)' "$run_dir/stdout" "$run_dir/stderr" "$run_dir/auxiliary" 2>/dev/null |
        grep -oE '[1-9][0-9]{1,8}' | sort -nu)
    for pid in $$ $controller_pid $pids; do
        [[ -d "/proc/$pid" ]] || continue
        for item in stat status cmdline environ cgroup limits; do
            [[ -r "/proc/$pid/$item" ]] && cp -- "/proc/$pid/$item" "$run_dir/proc/${pid}-${item}" 2>/dev/null || true
        done
        readlink "/proc/$pid/exe" > "$run_dir/proc/${pid}-exe" 2>&1 || true
    done
    printf 'FAIL mode=%s iteration=%s exit=%s reason=%s evidence=%s failed=%s\n' \
        "$mode" "$iteration" "$exit_code" "$reason" "$run_dir" "${failed_tests:-unparsed}"
}

failures=0
completed=0
maximum_processes=0
maximum_sandboxes=0
tests_executed=0
started_batch=$(date +%s%3N)
for (( iteration = 1; iteration <= runs; iteration++ )); do
    run_id="$(date -u +%Y%m%dT%H%M%S.%NZ)-${mode}-${threads}-${iteration}-$$"
    tmp="$batch_dir/.running-$run_id"
    mkdir -p "$tmp"
    if [[ -n "$test_binary" ]]; then
        args=("$test_binary")
    else
        args=(cargo test --locked --test native_process_control_tests)
    fi
    [[ -n "$filter" && "$filter" != "@launcher-watchdog" ]] && args+=("$filter")
    [[ -z "$test_binary" ]] && args+=(--)
    [[ -n "$filter" && "$filter" != "@launcher-watchdog" ]] && args+=(--exact)
    if [[ "$filter" == "@launcher-watchdog" ]]; then
        for skipped in \
            caminhos_nativos_mapeados_usam_a_autoridade_controlada \
            cem_execucoes_pequenas_nao_acumulam_filhos_nem_temporarios \
            execution_root_symlink_e_entradas_symlink_nunca_escapam \
            parser_proc_rejeita_truncado_nao_numerico_e_ambiguidade \
            proc_stat_comm_complexo_usa_starttime_real \
            raiz_real_ausente_existente_e_segunda_execucao_sao_idempotentes \
            sensibilidade_detecta_variacoes_e_restaura_fontes_byte_a_byte \
            target_symlink_bloqueia_antes_de_entregar_ambiente_ao_filho \
            troca_da_raiz_antes_do_cleanup_falha_fechada_e_preserva_conteudo
        do
            args+=(--skip "$skipped")
        done
    fi
    [[ "$threads" != default ]] && args+=("--test-threads=$threads")
    start=$(date +%s%3N)
    if [[ -n "$test_binary" ]]; then
        invocation=("${args[@]}")
    else
        invocation=("$repo_root/ci_env.sh" "${args[@]}")
    fi
    # -----------------------------------------------------------------------
    # Janela critica de inicializacao.
    #
    # Comeca antes da linha que inicia o controlador e so termina depois de PID,
    # start time, PGID e SID capturados, revalidados e promovidos. Todo ponto
    # dentro dela tem comportamento definido: o sinal e diferido, registrado e
    # processado depois, nunca perdido e nunca aplicado a identidade incompleta.
    # -----------------------------------------------------------------------
    active_state=starting
    monitor_pid=
    controller_pid=
    if ! pinker_flake_open_identity_channel; then
        printf 'pinker-flake-runner: canal de prontidao indisponivel: %s\n' \
            "$identity_channel" >&2
        pinker_flake_fail_identity
    fi
    pinker_flake_startup_hook before-spawn
    # Sinal antes do spawn: nada foi criado, e nada deve ser.
    pinker_flake_settle_pending
    PINKER_FLAKE_IDENTITY_CHANNEL="$identity_channel" \
        setsid bash -c "$PINKER_FLAKE_CONTROLLER_PREAMBLE" pinker-flake-controller \
        timeout --signal=TERM --kill-after=5s "${per_run_timeout}s" "${invocation[@]}" \
        > "$tmp/stdout" 2> "$tmp/stderr" &
    controller_pid=$!
    active_pid=$controller_pid
    pinker_flake_startup_hook after-spawn
    if ! pinker_flake_capture_identity; then
        printf 'pinker-flake-runner: identidade do controlador nao pode ser provada\n' >&2
        printf 'pinker-flake-runner: controller_pid=%s iteration=%s\n' \
            "$controller_pid" "$iteration" >&2
        pinker_flake_fail_identity
    fi
    pinker_flake_close_identity_channel
    pinker_flake_startup_hook after-identity
    active_state=active
    pinker_flake_startup_hook after-active
    pinker_flake_settle_pending
    # --- fim da janela critica ---------------------------------------------
    (
        while kill -0 "$controller_pid" 2>/dev/null; do
            process_count=$(ps -eo args= | awk '/native_process_control_tests/ && !/awk/ { count++ } END { print count + 0 }')
            sandbox_count=$(find "$repo_root/target/pinker-exec" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l)
            printf '%s %s\n' "$process_count" "$sandbox_count"
            sleep 0.05
        done
    ) > "$tmp/resource-samples.txt" &
    monitor_pid=$!
    pinker_flake_startup_hook after-monitor
    pinker_flake_settle_pending
    wait "$controller_pid"
    exit_code=$?
    active_state=reaping
    active_pid=
    active_start=
    active_pgid=
    active_sid=
    pinker_flake_reap_monitor
    active_state=finished
    end=$(date +%s%3N)
    duration=$((end-start))
    run_max_processes=$(awk 'BEGIN { max=0 } $1 > max { max=$1 } END { print max }' "$tmp/resource-samples.txt")
    run_max_sandboxes=$(awk 'BEGIN { max=0 } $2 > max { max=$2 } END { print max }' "$tmp/resource-samples.txt")
    (( run_max_processes > maximum_processes )) && maximum_processes=$run_max_processes
    (( run_max_sandboxes > maximum_sandboxes )) && maximum_sandboxes=$run_max_sandboxes

    # ---------------------------------------------------------------------
    # Veredito da iteracao.
    #
    # O codigo de saida do harness e necessario, nunca suficiente. Uma
    # iteracao so e PASS quando o harness termina em zero, existe resumo
    # reconhecido pertencente a esta execucao, e pelo menos um teste foi
    # efetivamente executado. Qualquer outra combinacao falha fechada e
    # preserva a evidencia.
    # ---------------------------------------------------------------------
    run_status=pass
    run_reason=ok
    run_executed=0
    run_summary_recognized=false
    if (( exit_code != 0 )); then
        run_status=fail
        run_reason=harness-exit-$exit_code
        if summary_line=$(pinker_flake_parse_summary "$tmp/stdout"); then
            run_summary_recognized=true
            read -r run_executed _run_passed _run_failed <<<"$summary_line"
        fi
    elif summary_line=$(pinker_flake_parse_summary "$tmp/stdout"); then
        run_summary_recognized=true
        read -r run_executed _run_passed _run_failed <<<"$summary_line"
        if (( run_executed == 0 )); then
            run_status=fail
            run_reason=no-tests-executed
        fi
    else
        run_status=fail
        run_reason=unparseable-test-summary
    fi
    tests_executed=$(( tests_executed + run_executed ))

    if [[ $run_status == pass ]]; then
        rm -rf -- "$tmp"
        printf 'PASS mode=%s iteration=%s/%s duration_ms=%s tests=%s\n' \
            "$mode" "$iteration" "$runs" "$duration" "$run_executed"
    else
        final="$batch_dir/$run_id"
        mv -- "$tmp" "$final"
        preserve_failure "$final" "$iteration" "$duration" "$exit_code" "$controller_pid" "$run_reason"
        failures=$((failures+1))
    fi
    completed=$((completed+1))
    printf 'mode=%s\ncompleted=%s\nruns=%s\nfailures=%s\ntests_executed=%s\nlast_duration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\nbatch_id=%s\nhead_sha=%s\n' \
        "$mode" "$completed" "$runs" "$failures" "$tests_executed" "$duration" "$maximum_processes" "$maximum_sandboxes" "$batch_id" "$head_sha" > "$progress_file"
done
ended_batch=$(date +%s%3N)

# Nenhuma execucao comprovada nunca produz resumo verde.
if (( completed == 0 )); then
    failures=$((failures+1))
fi

batch_exit=$(pinker_flake_exit_code_for "$failures")
printf 'mode=%s\nruns=%s\ncompleted=%s\nfailures=%s\ntests_executed=%s\nduration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\nevidence_root=%s\nbatch_id=%s\nhead_sha=%s\nbatch_dir=%s\nexit_code=%s\n' \
    "$mode" "$runs" "$completed" "$failures" "$tests_executed" "$((ended_batch-started_batch))" "$maximum_processes" "$maximum_sandboxes" "$evidence_root" "$batch_id" "$head_sha" "$batch_dir" "$batch_exit" > "$summary_file"

# Projecao legada, publicada somente agora. Um lote interrompido nunca chega
# aqui e portanto jamais substitui o ultimo resumo completo. Um lote falhado
# publica o proprio resumo, com `failures` maior que zero e `exit_code` nao
# zero: a projecao nunca fica verde por conta de outro lote.
pinker_flake_publish_legacy "$summary_file" "$legacy_summary" ||
    printf 'pinker-flake-runner: falha ao publicar projecao legada do resumo\n' >&2
pinker_flake_publish_legacy "$progress_file" "$legacy_progress" ||
    printf 'pinker-flake-runner: falha ao publicar projecao legada do progresso\n' >&2

printf 'SUMMARY mode=%s runs=%s completed=%s failures=%s tests_executed=%s duration_ms=%s batch_id=%s head_sha=%s evidence_root=%s\n' \
    "$mode" "$runs" "$completed" "$failures" "$tests_executed" "$((ended_batch-started_batch))" "$batch_id" "$head_sha" "$batch_dir"
exit "$batch_exit"
