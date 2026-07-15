---
pinker-doc: 1
id: engine
domain: engine
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - engine.territory
related:
  - roadmap
  - history
  - rosa
---

# Engine — hemisfério factual e operacional

- **Classe:** Engine
- **Papel:** navegação
- **Status:** ativo

Portal do território **Engine**. Ele reúne o estado real implementado do
compilador, do runtime e do backend. Os documentos canônicos permanecem na raiz
de `docs/` (migração gradual, sem reorganização global destrutiva); este portal
apenas dá território e rotas a eles.

## Propósito

Preservar a verdade factual e operacional da Pinker: o que está pronto hoje,
qual fase e bloco estão ativos, e como o pipeline é organizado.

## Escopo

- estado operacional e handoff da rodada;
- regras de atualização documental;
- mapa de código e organização do pipeline;
- inventário técnico (não roadmap).

## Fora do escopo

- identidade, voz e visão → território **Rosa** (`../rosa/README.md`);
- ordem ativa e blocos → território **Roadmap** (`../roadmap/README.md`);
- crônica histórica → território **Histórico** (`../history/README.md`).

## Autoridade

Engine governa o estado factual corrente. Não decide direção identitária (Rosa)
nem inventa história (Histórico).

## Mapa

| Necessidade | Documento |
|---|---|
| estado operacional e handoff | `../handoff_codex.md` |
| regras de atualização documental | `../doc_rules.md` |
| mapa de código por feature | `../code_map.md` |
| inventário técnico futuro | `../future.md` |
| manual de uso implementado | `../../MANUAL.md` |

## Rotas de leitura

### Situar-se no estado atual
1. `../handoff_codex.md`
2. `../roadmap/README.md`

### Preparar uma mudança funcional
1. `../doc_rules.md`
2. `../code_map.md`

## Ciclo de vida

O estado corrente muda por mudança funcional real com evidência em código,
testes e docs canônicos; tarefa operacional não abre fase nem reescreve história.

## Saídas

- **Roadmap:** `../roadmap/README.md`
- **Histórico:** `../history/README.md`
- **Rosa:** `../rosa/README.md`
