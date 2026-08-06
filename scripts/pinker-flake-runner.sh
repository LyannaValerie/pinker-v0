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

active_pid=
active_start=

active_identity_matches() {
    local stat rest
    [[ -n "$active_pid" && -n "$active_start" ]] || return 1
    [[ -r "/proc/$active_pid/stat" ]] || return 1
    stat=$(<"/proc/$active_pid/stat") || return 1
    rest=${stat##*) }
    set -- $rest
    [[ "${3:-}" == "$active_pid" && "${20:-}" == "$active_start" ]]
}

cleanup_active() {
    active_identity_matches || return 0
    kill -TERM -- "-$active_pid" 2>/dev/null || true
    local _attempt
    for (( _attempt = 0; _attempt < 100; _attempt++ )); do
        active_identity_matches || break
        sleep 0.02
    done
    if active_identity_matches; then
        kill -KILL -- "-$active_pid" 2>/dev/null || true
    fi
    wait "$active_pid" 2>/dev/null || true
    active_pid=
    active_start=
}

interrupt_runner() {
    cleanup_active
    if [[ -n "${tmp:-}" && -d "${tmp:-}" ]]; then
        local interrupted_name interrupted_dir stopped_at interrupted_duration
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
    fi
    exit "$PINKER_FLAKE_EXIT_INTERRUPTED"
}

# A liberacao do lock fica no EXIT porque `interrupt_runner` termina por `exit`:
# sucesso, falha comum, erro apos a aquisicao, SIGINT, SIGTERM e SIGHUP passam
# todos por aqui. SIGKILL nao executa trap algum, e por isso o lock registra
# identidade suficiente para que uma campanha posterior o classifique e o
# recupere.
trap 'cleanup_active; pinker_flake_release_lock' EXIT
trap interrupt_runner INT TERM HUP

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
    setsid timeout --signal=TERM --kill-after=5s "${per_run_timeout}s" "${invocation[@]}" > "$tmp/stdout" 2> "$tmp/stderr" &
    controller_pid=$!
    active_pid=$controller_pid
    active_start=
    for (( _probe = 0; _probe < 50; _probe++ )); do
        active_start=$(proc_start_time "$active_pid" 2>/dev/null || true)
        [[ -n "$active_start" ]] && break
        kill -0 "$active_pid" 2>/dev/null || break
        sleep 0.01
    done
    (
        while kill -0 "$controller_pid" 2>/dev/null; do
            process_count=$(ps -eo args= | awk '/native_process_control_tests/ && !/awk/ { count++ } END { print count + 0 }')
            sandbox_count=$(find "$repo_root/target/pinker-exec" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l)
            printf '%s %s\n' "$process_count" "$sandbox_count"
            sleep 0.05
        done
    ) > "$tmp/resource-samples.txt" &
    monitor_pid=$!
    wait "$controller_pid"
    exit_code=$?
    active_pid=
    active_start=
    wait "$monitor_pid" 2>/dev/null || true
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
