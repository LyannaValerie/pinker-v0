#!/usr/bin/env bash
set -euo pipefail

mode=dry-run
older_than=3600

usage() {
    printf '%s\n' 'uso: scripts/pinker-cleanup.sh [--dry-run|--apply] [--older-than SEGUNDOS]'
}

while (($# > 0)); do
    case "$1" in
        --dry-run) mode=dry-run; shift ;;
        --apply) mode=apply; shift ;;
        --older-than)
            if (($# < 2)) || [[ ! "$2" =~ ^[0-9]+$ ]]; then
                printf '%s\n' 'erro: --older-than exige segundos inteiros não negativos' >&2
                exit 2
            fi
            older_than=$2
            shift 2
            ;;
        --help|-h) usage; exit 0 ;;
        *)
            printf 'erro: argumento desconhecido: %q\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd -P)
target_root="$repo_root/target"
execution_root="$target_root/pinker-exec"
test_hook_fd=

if [[ -L /proc/$$/fd/9 ]]; then
    hook_target=$(readlink -- "/proc/$$/fd/9" 2>/dev/null || true)
    if [[ "$hook_target" == socket:* ]]; then
        hook_magic=
        if IFS= read -r -t 0.1 hook_magic <&9 \
            && [[ "$hook_magic" == PINKER_INTERNAL_CLEANUP_TEST_V1 ]]; then
            test_hook_fd=9
        fi
    fi
fi

run_test_hook() {
    local stage=$1 original=$2 quarantine=$3 acknowledgement
    [[ -n "$test_hook_fd" ]] || return 0
    printf '%s\t%s\t%s\n' "$stage" "$original" "$quarantine" >&9
    IFS= read -r acknowledgement <&9 || return 1
    [[ "$acknowledgement" == OK ]]
}

root_error() {
    printf 'ERROR root %s\n' "$1" >&2
    exit 1
}

if [[ -L "$target_root" ]]; then root_error 'target-is-symlink'; fi
if [[ ! -e "$target_root" ]]; then exit 0; fi
if [[ ! -d "$target_root" ]]; then root_error 'target-not-real-directory'; fi
if [[ -L "$execution_root" ]]; then root_error 'execution-root-is-symlink'; fi
if [[ ! -e "$execution_root" ]]; then exit 0; fi
if [[ ! -d "$execution_root" ]]; then root_error 'execution-root-not-real-directory'; fi

canonical_root=$(realpath -e -- "$execution_root") || root_error 'canonicalize'
case "$canonical_root/" in
    "$repo_root"/*/) ;;
    *) root_error 'outside-authorized-repository' ;;
esac
root_identity=$(stat -Lc '%d:%i' -- "$execution_root") || root_error 'identity'

revalidate_root() {
    local current current_identity
    [[ ! -L "$target_root" && -d "$target_root" ]] || return 1
    [[ ! -L "$execution_root" && -d "$execution_root" ]] || return 1
    current=$(realpath -e -- "$execution_root") || return 1
    [[ "$current" == "$canonical_root" ]] || return 1
    case "$current/" in
        "$repo_root"/*/) ;;
        *) return 1 ;;
    esac
    current_identity=$(stat -Lc '%d:%i' -- "$execution_root") || return 1
    [[ "$current_identity" == "$root_identity" ]]
}

marker_reason=invalid-marker
parse_marker() {
    local marker=$1 expected_name_owner=$2 expected_identity=$3
    local line key value
    local -A parsed=()
    marker_reason=invalid-marker
    while IFS= read -r line; do
        if [[ ! "$line" =~ ^([a-z0-9_]+):\ (.+)$ ]]; then return 1; fi
        key=${BASH_REMATCH[1]}
        value=${BASH_REMATCH[2]}
        case "$key" in
            schema|owner_pid|owner_start_time|execution_device|execution_inode|launcher_pid|launcher_start_time|guest_pid|process_group_id|watchdog_pid|created_at_unix|git_head|executable_sha256|state) ;;
            *) return 1 ;;
        esac
        if [[ -v "parsed[$key]" ]]; then return 1; fi
        parsed[$key]=$value
    done < "$marker"
    if [[ ${parsed[schema]-} == 1 ]]; then
        marker_reason=legacy-marker
        return 1
    fi
    ((${#parsed[@]} == 14)) || return 1
    schema=${parsed[schema]-}
    owner_pid=${parsed[owner_pid]-}
    owner_start=${parsed[owner_start_time]-}
    execution_device=${parsed[execution_device]-}
    execution_inode=${parsed[execution_inode]-}
    launcher_pid=${parsed[launcher_pid]-}
    launcher_start=${parsed[launcher_start_time]-}
    guest_pid=${parsed[guest_pid]-}
    process_group_id=${parsed[process_group_id]-}
    watchdog_pid=${parsed[watchdog_pid]-}
    created=${parsed[created_at_unix]-}
    git_head=${parsed[git_head]-}
    executable_hash=${parsed[executable_sha256]-}
    state=${parsed[state]-}
    [[ "$schema" == 2 ]] || return 1
    [[ "$owner_pid" =~ ^[1-9][0-9]*$ && "$owner_start" =~ ^[1-9][0-9]*$ ]] || return 1
    [[ "$execution_device" =~ ^[0-9]+$ && "$execution_inode" =~ ^[1-9][0-9]*$ ]] || return 1
    [[ "$created" =~ ^[0-9]+$ ]] || return 1
    [[ "$launcher_pid" =~ ^(null|[1-9][0-9]*)$ ]] || return 1
    [[ "$launcher_start" =~ ^(null|[1-9][0-9]*)$ ]] || return 1
    [[ "$guest_pid" =~ ^(null|[1-9][0-9]*)$ ]] || return 1
    [[ "$process_group_id" =~ ^(null|[1-9][0-9]*)$ ]] || return 1
    [[ "$watchdog_pid" =~ ^(null|[1-9][0-9]*)$ ]] || return 1
    [[ "$git_head" =~ ^(unknown|[0-9a-f]{40})$ ]] || return 1
    [[ "$executable_hash" =~ ^(pending|unknown|[0-9a-f]{64})$ ]] || return 1
    [[ "$owner_pid" == "$expected_name_owner" ]] || { marker_reason=name-owner-mismatch; return 1; }
    [[ "$execution_device:$execution_inode" == "$expected_identity" ]] || { marker_reason=identity-mismatch; return 1; }

    local shape=invalid
    if [[ "$launcher_pid" == null && "$launcher_start" == null && "$guest_pid" == null \
        && "$process_group_id" == null && "$watchdog_pid" == null ]]; then
        shape=preparing
    elif [[ "$launcher_pid" != null && "$launcher_start" != null && "$guest_pid" == null \
        && "$process_group_id" == "$launcher_pid" && "$watchdog_pid" == null ]]; then
        shape=launcher-ready
    elif [[ "$launcher_pid" != null && "$launcher_start" != null && "$guest_pid" == null \
        && "$process_group_id" == "$launcher_pid" && "$watchdog_pid" != null ]]; then
        shape=watchdog-ready
    elif [[ "$launcher_pid" != null && "$launcher_start" != null && "$guest_pid" != null \
        && "$process_group_id" == "$launcher_pid" && "$watchdog_pid" != null ]]; then
        shape=running
    fi
    case "$state:$shape" in
        preparing:preparing|launcher-ready:launcher-ready|watchdog-ready:watchdog-ready|running:running|terminating:running|finished:running|failed:preparing|failed:launcher-ready|failed:watchdog-ready|failed:running) ;;
        *) return 1 ;;
    esac
    case "$state" in
        preparing|launcher-ready|watchdog-ready) [[ "$executable_hash" == pending ]] || return 1 ;;
        running|terminating|finished) [[ "$executable_hash" != pending ]] || return 1 ;;
    esac
    return 0
}

read_proc_start_time() {
    local pid=$1 stat_text suffix
    local -a suffix_fields=()
    [[ -d /proc ]] || return 2
    if ! IFS= read -r stat_text 2>/dev/null < "/proc/$pid/stat"; then
        if [[ -e "/proc/$pid" ]]; then return 2; fi
        return 1
    fi
    [[ "$stat_text" == *") "* ]] || return 2
    suffix=${stat_text##*) }
    [[ "$suffix" != "$stat_text" ]] || return 2
    read -r -a suffix_fields <<< "$suffix"
    ((${#suffix_fields[@]} >= 20)) || return 2
    [[ "${suffix_fields[0]}" =~ ^.$ ]] || return 2
    [[ "${suffix_fields[19]}" =~ ^[0-9]+$ ]] || return 2
    printf '%s' "${suffix_fields[19]}"
}

now=$(date +%s)
partial_error=0
shopt -s nullglob
candidates=("$execution_root"/exec-*)
shopt -u nullglob

for directory in "${candidates[@]}"; do
    name=${directory##*/}
    if [[ -L "$directory" || ! -d "$directory" || ! "$name" =~ ^exec-([0-9]+)-([0-9]+)$ ]]; then
        printf 'PRESERVED invalid-entry %q\n' "$name"
        continue
    fi
    name_owner=${BASH_REMATCH[1]}
    entry_identity=$(stat -Lc '%d:%i' -- "$directory") || {
        printf 'PRESERVED unreadable-identity %q\n' "$name"
        continue
    }

    canonical_directory=$(realpath -e -- "$directory") || {
        printf 'ERROR canonicalize %q\n' "$name" >&2
        partial_error=1
        continue
    }
    case "$canonical_directory/" in
        "$canonical_root"/*/) ;;
        *)
            printf 'ERROR outside-root %q\n' "$name" >&2
            partial_error=1
            continue
            ;;
    esac

    marker="$directory/owner.marker"
    if [[ -L "$marker" || ! -f "$marker" ]]; then
        printf 'PRESERVED missing-marker %q\n' "$name"
        continue
    fi

    if ! parse_marker "$marker" "$name_owner" "$entry_identity"; then
        printf 'PRESERVED %s %q\n' "$marker_reason" "$name"
        continue
    fi

    if ((created > now || now - created < older_than)); then
        printf 'PRESERVED too-young %q\n' "$name"
        continue
    fi

    current_start=''
    if current_start=$(read_proc_start_time "$owner_pid"); then
        if [[ "$current_start" == "$owner_start" ]]; then
            printf 'PRESERVED live-owner %q\n' "$name"
            continue
        fi
    else
        proc_result=$?
        if ((proc_result != 1)); then
            printf 'PRESERVED ownership-unknown %q\n' "$name"
            continue
        fi
    fi

    if [[ "$mode" == dry-run ]]; then
        printf 'STALE dry-run %q\n' "$name"
        continue
    fi

    if ! revalidate_root; then
        printf 'ERROR root changed-before-quarantine %q\n' "$name" >&2
        exit 1
    fi
    if [[ -L "$directory" || ! -d "$directory" ]]; then
        printf 'PRESERVED changed-entry %q\n' "$name"
        continue
    fi
    quarantine_counter=0
    while :; do
        quarantine="$execution_root/.pinker-quarantine-$$-${RANDOM}-${quarantine_counter}"
        if [[ ! -e "$quarantine" && ! -L "$quarantine" ]]; then break; fi
        ((quarantine_counter += 1))
        if ((quarantine_counter > 1024)); then
            printf 'PRESERVED quarantine-exhausted %q\n' "$name"
            continue 2
        fi
    done
    if ! run_test_hook before-quarantine "$directory" "$quarantine"; then
        printf 'ERROR test-hook-before %q\n' "$name" >&2
        exit 1
    fi
    if ! mv -T -n -- "$directory" "$quarantine"; then
        printf 'ERROR quarantine %q\n' "$name" >&2
        partial_error=1
        continue
    fi
    if [[ -e "$directory" || -L "$directory" ]]; then
        printf 'PRESERVED quarantine-exists %q\n' "$name"
        continue
    fi
    if ! run_test_hook after-quarantine "$directory" "$quarantine"; then
        printf 'ERROR test-hook-after %q\n' "$name" >&2
        exit 1
    fi
    quarantined_identity=$(stat -Lc '%d:%i' -- "$quarantine" 2>/dev/null) || {
        printf 'PRESERVED identity-mismatch %q\n' "$name"
        continue
    }
    if [[ -L "$quarantine" || ! -d "$quarantine" || "$quarantined_identity" != "$entry_identity" ]]; then
        printf 'PRESERVED identity-mismatch %q\n' "$name"
        continue
    fi
    if ! revalidate_root; then
        printf 'ERROR root changed-after-quarantine %q\n' "$name" >&2
        exit 1
    fi
    rm -rf -- "$quarantine" || {
        printf 'ERROR remove-quarantine %q\n' "$name" >&2
        partial_error=1
        continue
    }
    if [[ -e "$quarantine" || -L "$quarantine" ]]; then
        printf 'ERROR quarantine-remained %q\n' "$name" >&2
        partial_error=1
        continue
    fi
    printf 'STALE removed %q\n' "$name"
done

exit "$partial_error"
