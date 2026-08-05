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
readonly PINKER_FLAKE_EXIT_INTERRUPTED=130

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

test_binary=${PINKER_FLAKE_TEST_BINARY:-}

# A partir daqui o uso esta validado e o efeito colateral e autorizado.
mkdir -p "$evidence_root"
progress_file="$evidence_root/PROGRESS-$mode.txt"
summary_file="$evidence_root/SUMMARY-$mode.txt"
rm -f -- "$progress_file" "$summary_file"

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
        interrupted_dir="$evidence_root/INTERRUPTED-$interrupted_name"
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

trap cleanup_active EXIT
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
    tmp="$evidence_root/.running-$run_id"
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
        final="$evidence_root/$run_id"
        mv -- "$tmp" "$final"
        preserve_failure "$final" "$iteration" "$duration" "$exit_code" "$controller_pid" "$run_reason"
        failures=$((failures+1))
    fi
    completed=$((completed+1))
    printf 'mode=%s\ncompleted=%s\nruns=%s\nfailures=%s\ntests_executed=%s\nlast_duration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\n' \
        "$mode" "$completed" "$runs" "$failures" "$tests_executed" "$duration" "$maximum_processes" "$maximum_sandboxes" > "$progress_file"
done
ended_batch=$(date +%s%3N)

# Nenhuma execucao comprovada nunca produz resumo verde.
if (( completed == 0 )); then
    failures=$((failures+1))
fi

batch_exit=$(pinker_flake_exit_code_for "$failures")
printf 'mode=%s\nruns=%s\ncompleted=%s\nfailures=%s\ntests_executed=%s\nduration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\nevidence_root=%s\nexit_code=%s\n' \
    "$mode" "$runs" "$completed" "$failures" "$tests_executed" "$((ended_batch-started_batch))" "$maximum_processes" "$maximum_sandboxes" "$evidence_root" "$batch_exit" > "$summary_file"
printf 'SUMMARY mode=%s runs=%s completed=%s failures=%s tests_executed=%s duration_ms=%s evidence_root=%s\n' \
    "$mode" "$runs" "$completed" "$failures" "$tests_executed" "$((ended_batch-started_batch))" "$evidence_root"
exit "$batch_exit"
