#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Uso:
  ./scripts/ci_env.sh --preflight
  ./scripts/ci_env.sh <comando> [args...]

Executa a suite oficial da Pinker v0 em ambiente saneado:
- remove RUSTFLAGS;
- remove CARGO_ENCODED_RUSTFLAGS;
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
    if command -v rustup >/dev/null 2>&1; then
        printf 'toolchain=%s\n' "$(rustup show active-toolchain)"
    else
        printf 'toolchain=rustup-unavailable\n'
    fi
    printf 'original_RUSTFLAGS=%q\n' "${RUSTFLAGS-}"
    printf 'original_CARGO_ENCODED_RUSTFLAGS=%q\n' "${CARGO_ENCODED_RUSTFLAGS-}"
    printf 'sanitized_RUSTFLAGS=%q\n' ''
    printf 'sanitized_CARGO_ENCODED_RUSTFLAGS=%q\n' ''
    exit 0
fi

if [[ $# -eq 0 ]]; then
    usage >&2
    exit 2
fi

unset RUSTFLAGS || true
unset CARGO_ENCODED_RUSTFLAGS || true

exec "$@"
