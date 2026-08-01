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

field() {
    local marker=$1 key=$2
    local -a values=()
    mapfile -t values < <(sed -n "s/^${key}: //p" "$marker")
    if ((${#values[@]} != 1)); then return 1; fi
    printf '%s' "${values[0]}"
}

read_proc_start_time() {
    local pid=$1 stat_text suffix
    local -a suffix_fields=()
    [[ -d /proc ]] || return 2
    if ! IFS= read -r stat_text < "/proc/$pid/stat"; then
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
    if [[ -L "$directory" || ! -d "$directory" || ! "$name" =~ ^exec-[0-9]+-[0-9]+$ ]]; then
        printf 'PRESERVED invalid-entry %q\n' "$name"
        continue
    fi

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

    schema=$(field "$marker" schema) || schema=''
    owner_pid=$(field "$marker" owner_pid) || owner_pid=''
    owner_start=$(field "$marker" owner_start_time) || owner_start=''
    created=$(field "$marker" created_at_unix) || created=''
    child_pid=$(field "$marker" child_pid) || child_pid=''
    child_pgid=$(field "$marker" child_pgid) || child_pgid=''
    supervisor_pid=$(field "$marker" supervisor_pid) || supervisor_pid=''
    git_head=$(field "$marker" git_head) || git_head=''
    executable_hash=$(field "$marker" executable_sha256) || executable_hash=''
    state=$(field "$marker" state) || state=''

    if [[ "$schema" != 1 || ! "$owner_pid" =~ ^[0-9]+$ || ! "$owner_start" =~ ^[0-9]+$ \
        || ! "$created" =~ ^[0-9]+$ \
        || ! "$child_pid" =~ ^(null|[0-9]+)$ \
        || ! "$child_pgid" =~ ^(null|-?[0-9]+)$ \
        || ! "$supervisor_pid" =~ ^(null|[0-9]+)$ \
        || ! "$git_head" =~ ^(unknown|[0-9a-f]{40})$ \
        || ! "$executable_hash" =~ ^(pending|unknown|[0-9a-f]{64})$ \
        || ! "$state" =~ ^(preparing|running|finished|failed)$ ]]; then
        printf 'PRESERVED invalid-marker %q\n' "$name"
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
        printf 'ERROR root changed-before-remove %q\n' "$name" >&2
        exit 1
    fi
    if [[ -L "$directory" || ! -d "$directory" ]]; then
        printf 'PRESERVED changed-entry %q\n' "$name"
        continue
    fi
    current_directory=$(realpath -e -- "$directory") || {
        printf 'ERROR recanonicalize %q\n' "$name" >&2
        partial_error=1
        continue
    }
    if [[ "$current_directory" != "$canonical_directory" ]]; then
        printf 'PRESERVED changed-entry %q\n' "$name"
        continue
    fi
    rm -rf -- "$directory" || {
        printf 'ERROR remove %q\n' "$name" >&2
        partial_error=1
        continue
    }
    if [[ -e "$directory" || -L "$directory" ]]; then
        printf 'ERROR remained %q\n' "$name" >&2
        partial_error=1
        continue
    fi
    printf 'STALE removed %q\n' "$name"
done

exit "$partial_error"
