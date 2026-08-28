#!/usr/bin/env bash
# Projeta a autoridade versionada da Forja para o host.
#
# INSTALLED_COPY != SOURCE_AUTHORITY. Este script existe para que a cópia em
# /opt/pinker/bin seja sempre uma projeção reproduzível da fonte versionada, e
# nunca uma variante editada no host. Uma reinstalação a partir do repositório
# tem de reconstruir exatamente a arquitetura corrente — nunca ressuscitar uma
# anterior.
#
#   ./scripts/forja/instalar.sh --check    verifica paridade fonte/instalado
#   sudo ./scripts/forja/instalar.sh       instala/atualiza a projeção
set -euo pipefail

RAIZ="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FONTE="$RAIZ/scripts/forja/forja_agentes.py"
DESTINO_DIR="${FORJA_INSTALL_DIR:-/opt/pinker/bin}"
DESTINO="$DESTINO_DIR/forja-agentes"
MANIFESTO="$DESTINO_DIR/.forja-agentes.provenance.json"

if [[ ! -f "$FONTE" ]]; then
    printf 'instalar: fonte ausente: %s\n' "$FONTE" >&2
    exit 2
fi

sha_fonte="$(sha256sum "$FONTE" | cut -d' ' -f1)"
commit="$(git -C "$RAIZ" rev-parse HEAD 2>/dev/null || printf 'UNKNOWN')"

if [[ "${1-}" == "--check" ]]; then
    if [[ ! -f "$DESTINO" ]]; then
        printf '{"status":"MISSING","installed":"%s","source_sha256":"%s"}\n' "$DESTINO" "$sha_fonte"
        exit 5
    fi
    sha_instalado="$(sha256sum "$DESTINO" | cut -d' ' -f1)"
    if [[ "$sha_instalado" != "$sha_fonte" ]]; then
        printf '{"status":"DRIFT","installed":"%s","installed_sha256":"%s","source_sha256":"%s"}\n' \
            "$DESTINO" "$sha_instalado" "$sha_fonte"
        exit 5
    fi
    printf '{"status":"PARITY","installed":"%s","sha256":"%s","source_commit":"%s"}\n' \
        "$DESTINO" "$sha_fonte" "$commit"
    exit 0
fi

if [[ ! -d "$DESTINO_DIR" ]]; then
    printf 'instalar: diretório de instalação ausente: %s\n' "$DESTINO_DIR" >&2
    exit 2
fi

install -m 0755 "$FONTE" "$DESTINO"
cat > "$MANIFESTO" <<JSON
{
  "schema": "forja-install-provenance-v1",
  "tool": "forja-agentes",
  "installed_path": "$DESTINO",
  "source_authority": "LyannaValerie/pinker-v0:scripts/forja/forja_agentes.py",
  "source_commit": "$commit",
  "source_sha256": "$sha_fonte",
  "installed_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
JSON
chmod 0644 "$MANIFESTO"
printf '{"status":"INSTALLED","installed":"%s","sha256":"%s","source_commit":"%s"}\n' \
    "$DESTINO" "$sha_fonte" "$commit"
