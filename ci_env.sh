#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${PINKER_BUILD_COMMIT-}" ]]; then
    ci_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PINKER_BUILD_COMMIT="$(git -C "$ci_repo_root" rev-parse HEAD 2>/dev/null || printf 'UNKNOWN')"
    export PINKER_BUILD_COMMIT
fi

if ! ulimit -S -c 0 2>/dev/null; then
    printf 'ci_env: não foi possível desabilitar core dumps para esta execução\n' >&2
    exit 1
fi

usage() {
    cat <<'EOF'
Uso:
  ./ci_env.sh --preflight
  ./ci_env.sh <comando> [args...]

Executa a suite oficial da Pinker v0 em ambiente saneado:
- remove RUSTFLAGS;
- remove CARGO_ENCODED_RUSTFLAGS;
- desabilita core dumps na árvore de processos;
- preserva toolchain stable configurada pelo projeto.
EOF
}

if [[ "${1-}" == "--help" ]]; then
    usage
    exit 0
fi

if [[ "${1-}" == "--preflight" ]]; then
    printf 'stable_only=1\n'
    printf 'cwd=%s\n' "$(pwd)"
    printf 'rustc=%s\n' "$(rustc --version)"
    printf 'cargo=%s\n' "$(cargo --version)"
    printf 'pinker_build_commit=%s\n' "$PINKER_BUILD_COMMIT"
    if command -v rustup >/dev/null 2>&1; then
        printf 'toolchain=%s\n' "$(rustup show active-toolchain)"
    else
        printf 'toolchain=rustup-unavailable\n'
    fi
    printf 'original_RUSTFLAGS=%q\n' "${RUSTFLAGS-}"
    printf 'original_CARGO_ENCODED_RUSTFLAGS=%q\n' "${CARGO_ENCODED_RUSTFLAGS-}"
    printf 'sanitized_RUSTFLAGS=%q\n' ''
    printf 'sanitized_CARGO_ENCODED_RUSTFLAGS=%q\n' ''
    printf 'core_dump_soft_limit=%s\n' "$(ulimit -c)"
    exit 0
fi

if [[ $# -eq 0 ]]; then
    usage >&2
    exit 2
fi

unset RUSTFLAGS || true
unset CARGO_ENCODED_RUSTFLAGS || true

exec "$@"
