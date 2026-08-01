#!/usr/bin/env bash
set -euo pipefail

mode=dry-run
older_than=3600

usage() {
    printf '%s\n' 'uso: scripts/pinker-cleanup.sh [--dry-run|--apply] [--older-than SEGUNDOS]'
}

while (($# > 0)); do
    case "$1" in
        --dry-run)
            mode=dry-run
            shift
            ;;
        --apply)
            mode=apply
            shift
            ;;
        --older-than)
            if (($# < 2)) || [[ ! "$2" =~ ^[0-9]+$ ]]; then
                printf '%s\n' 'erro: --older-than exige segundos inteiros não negativos' >&2
                exit 2
            fi
            older_than=$2
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        *)
            printf 'erro: argumento desconhecido: %q\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

script_dir=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
repo_root=$(CDPATH= cd -- "$script_dir/.." && pwd -P)
execution_root="$repo_root/target/pinker-exec"

if [[ ! -e "$execution_root" ]]; then
    exit 0
fi
if [[ -L "$execution_root" || ! -d "$execution_root" ]]; then
    printf '%s\n' 'erro: target/pinker-exec deve ser um diretório real' >&2
    exit 1
fi

canonical_root=$(realpath -e -- "$execution_root")
now=$(date +%s)
partial_error=0

field() {
    local marker=$1 key=$2 value
    mapfile -t value < <(sed -n "s/^${key}: //p" "$marker")
    if ((${#value[@]} != 1)); then
        return 1
    fi
    printf '%s' "${value[0]}"
}

mapfile -d '' candidates < <(
    find "$execution_root" -mindepth 1 -maxdepth 1 -type d -name 'exec-*' -print0 \
        | sort -z
)

for directory in "${candidates[@]}"; do
    name=${directory##*/}
    if [[ -L "$directory" || ! "$name" =~ ^exec-[0-9]+-[0-9]+$ ]]; then
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
    git_head=$(field "$marker" git_head) || git_head=''
    executable_hash=$(field "$marker" executable_sha256) || executable_hash=''
    state=$(field "$marker" state) || state=''

    if [[ "$schema" != 1 || ! "$owner_pid" =~ ^[0-9]+$ || ! "$owner_start" =~ ^[0-9]+$ \
        || ! "$created" =~ ^[0-9]+$ \
        || ! "$child_pid" =~ ^(null|[0-9]+)$ \
        || ! "$child_pgid" =~ ^(null|-?[0-9]+)$ \
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

    if [[ -r "/proc/$owner_pid/stat" ]]; then
        current_start=$(awk '{print $22}' "/proc/$owner_pid/stat" 2>/dev/null || true)
        if [[ "$current_start" == "$owner_start" ]]; then
            printf 'PRESERVED live-owner %q\n' "$name"
            continue
        fi
    fi

    if [[ "$mode" == dry-run ]]; then
        printf 'STALE %q\n' "$name"
        continue
    fi

    rm -rf -- "$canonical_directory" || {
        printf 'ERROR remove %q\n' "$name" >&2
        partial_error=1
        continue
    }
    if [[ -e "$directory" ]]; then
        printf 'ERROR remained %q\n' "$name" >&2
        partial_error=1
        continue
    fi
    printf 'REMOVED %q\n' "$name"
done

exit "$partial_error"
