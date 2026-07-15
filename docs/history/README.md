---
pinker-doc: 1
id: history
domain: history
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - history.territory
related:
  - engine
  - roadmap
---

# Histórico — território da crônica factual

- **Classe:** Engine
- **Papel:** navegação
- **Status:** ativo

Portal do território **Histórico**. O ponteiro canônico curto é `../history.md`;
o hub principal e os shards já vivem neste diretório. A crônica antiga permanece
intocada — a Trama Pinker não faz backfill retroativo.

## Propósito

Registrar o que aconteceu (fases, hotfixes, rodadas documentais e paralelas) sem
contaminar estado corrente, roadmap ou visão.

## Escopo

- índice histórico e shards por faixa;
- fases, hotfixes, documentação e fases paralelas.

## Fora do escopo

- estado corrente → território **Engine**;
- ordem futura → território **Roadmap**.

## Autoridade

O Histórico é proprietário da crônica factual passada. Não decide o presente nem
o futuro, e não é reescrito por inércia.

## Mapa

| Necessidade | Documento |
|---|---|
| ponteiro canônico curto | `../history.md` |
| hub principal de navegação | `indice.md` |
| fases históricas | `phases/` |
| hotfixes | `hotfixes/` |
| rodadas documentais | `documentation/` |
| fases paralelas | `parallel_phases/` |

## Rotas de leitura

### Reconstituir uma fase
1. `indice.md`
2. `phases/indice.md`

## Ciclo de vida

Uma entrada histórica é acrescentada quando um evento fecha; nunca é apagada por
tarefa operacional, e mudanças puramente numéricas de linha não geram evento.

## Saídas

- **Engine:** `../engine/README.md`
- **Roadmap:** `../roadmap/README.md`
