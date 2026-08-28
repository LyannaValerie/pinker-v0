#!/usr/bin/env bash
# Gate: nenhum arquivo versionado pode ensinar o layout aposentado da Forja.
#
# O layout anterior espalhava os recursos de uma Task por sete raízes irmãs sob
# /pinker. A arquitetura corrente dá a cada Task UM root observado sob
# `agentes/`. Este gate existe para que a topologia antiga não volte por
# documentação, template, script ou fixture esquecida — o modo mais comum de
# uma migração meio-feita se disfarçar de concluída.
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$RAIZ"

# Caminhos aposentados. Um agente que leia qualquer um destes como atual está
# operando a arquitetura errada.
APOSENTADOS=(
    '/pinker/worktrees'
    '/pinker/work/tasks'
    '/pinker/work/amara'
    '/pinker/work/velina'
    '/pinker/caches/target'
    '/pinker/build/tasks'
    '/pinker/scratch/agents'
    '/pinker/artifacts/tasks'
    '/pinker/state/tasks'
    '/pinker/msg'
    '/pinker/repos/'
    '/pinker/legado'
)

# `--untracked` é essencial: sem ele o gate só enxerga o que já foi commitado, e
# um arquivo novo de uma PR — exatamente onde a regressão nasce — passaria.
#
# Este próprio arquivo cita os caminhos por dever de ofício.
IGNORAR_ARQUIVOS='^scripts/forja/verificar-paths\.sh$'

falhas=0
for padrao in "${APOSENTADOS[@]}"; do
    while IFS= read -r linha; do
        [[ -z "$linha" ]] && continue
        arquivo="${linha%%:*}"
        if [[ "$arquivo" =~ $IGNORAR_ARQUIVOS ]]; then
            continue
        fi
        printf 'PATH_APOSENTADO %s\n' "$linha"
        falhas=$((falhas + 1))
    done < <(git grep -n -F --untracked --exclude-standard -- "$padrao" -- . 2>/dev/null || true)
done

# A raiz corrente precisa estar presente e ser ensinada em algum lugar.
if ! git grep -q -F --untracked --exclude-standard -- '/pinker/repo/pinker-v0' -- AGENTS.md; then
    printf 'AGENTS_NAO_ENSINA_CANONICAL_MAIN\n'
    falhas=$((falhas + 1))
fi
if ! git grep -q -F --untracked --exclude-standard -- 'forja-agentes' -- AGENTS.md; then
    printf 'AGENTS_NAO_ENSINA_O_OBSERVADOR\n'
    falhas=$((falhas + 1))
fi

if (( falhas > 0 )); then
    printf 'verificar-paths: %d referência(s) ao layout aposentado\n' "$falhas" >&2
    exit 1
fi
printf 'verificar-paths: OK — nenhuma referência ao layout aposentado\n'
