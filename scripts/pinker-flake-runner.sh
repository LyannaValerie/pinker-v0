#!/usr/bin/env bash
set -u
set -o pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd -P)
evidence_root="$repo_root/target/pinker-flake-evidence"
mode=${1:?mode required}
runs=${2:?run count required}
threads=${3:-default}
filter=${4:-}
per_run_timeout=${PINKER_FLAKE_RUN_TIMEOUT_SECONDS:-300}
test_binary=${PINKER_FLAKE_TEST_BINARY:-}
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
    for _ in $(seq 1 100); do
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
            130 \
            "${controller_pid:-0}"
    fi
    exit 130
}

trap cleanup_active EXIT
trap interrupt_runner INT TERM HUP

preserve_failure() {
    local run_dir=$1 iteration=$2 duration=$3 exit_code=$4 controller_pid=$5
    local failed_tests pids pid
    mkdir -p "$run_dir/proc"
    failed_tests=$(sed -n 's/^---- \(.*\) stdout ----$/\1/p; s/^    \([^ ]*\)\r\{0,1\}$/\1/p' "$run_dir/stdout" | sort -u)
    {
        printf 'iteration=%s\nmode=%s\ntest_filter=%s\ntest_threads=%s\n' "$iteration" "$mode" "${filter:-<complete-file>}" "$threads"
        printf 'runner_pid=%s\nrunner_start_time=%s\nharness_pid=%s\nharness_start_time=%s\n' "$$" "$(proc_start_time $$ 2>/dev/null || printf unknown)" "$controller_pid" "$(proc_start_time "$controller_pid" 2>/dev/null || printf exited)"
        printf 'duration_ms=%s\nexit_code=%s\nhead_git=%s\n' "$duration" "$exit_code" "$(git -C "$repo_root" rev-parse HEAD 2>/dev/null || printf unknown)"
        printf 'compiled_test_binary=%s\n' "${test_binary:-false}"
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
    printf 'FAIL mode=%s iteration=%s exit=%s evidence=%s failed=%s\n' "$mode" "$iteration" "$exit_code" "$run_dir" "${failed_tests:-unparsed}"
}

failures=0
maximum_processes=0
maximum_sandboxes=0
tests_executed=0
started_batch=$(date +%s%3N)
for iteration in $(seq 1 "$runs"); do
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
    for _ in $(seq 1 50); do
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
    run_tests=$(sed -n 's/^test result: .* \([0-9][0-9]*\) passed.*/\1/p' "$tmp/stdout" | tail -n 1)
    [[ "$run_tests" =~ ^[0-9]+$ ]] && tests_executed=$((tests_executed+run_tests))
    if [[ "$exit_code" -eq 0 ]]; then
        rm -rf -- "$tmp"
        printf 'PASS mode=%s iteration=%s/%s duration_ms=%s\n' "$mode" "$iteration" "$runs" "$duration"
    else
        final="$evidence_root/$run_id"
        mv -- "$tmp" "$final"
        preserve_failure "$final" "$iteration" "$duration" "$exit_code" "$controller_pid"
        failures=$((failures+1))
    fi
    printf 'mode=%s\ncompleted=%s\nruns=%s\nfailures=%s\ntests_executed=%s\nlast_duration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\n' \
        "$mode" "$iteration" "$runs" "$failures" "$tests_executed" "$duration" "$maximum_processes" "$maximum_sandboxes" > "$progress_file"
done
ended_batch=$(date +%s%3N)
printf 'mode=%s\nruns=%s\nfailures=%s\ntests_executed=%s\nduration_ms=%s\nmaximum_processes=%s\nmaximum_sandboxes=%s\nevidence_root=%s\n' \
    "$mode" "$runs" "$failures" "$tests_executed" "$((ended_batch-started_batch))" "$maximum_processes" "$maximum_sandboxes" "$evidence_root" > "$summary_file"
printf 'SUMMARY mode=%s runs=%s failures=%s duration_ms=%s evidence_root=%s\n' "$mode" "$runs" "$failures" "$((ended_batch-started_batch))" "$evidence_root"
exit "$failures"
