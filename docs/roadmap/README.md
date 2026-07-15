---
pinker-doc: 1
id: roadmap
domain: roadmap
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - roadmap.territory
related:
  - engine
  - history
---

# Roadmap — território da ordem ativa

- **Classe:** Engine
- **Papel:** navegação
- **Status:** ativo

Portal do território **Roadmap**. A ordem ativa oficial vive em `../roadmap.md`;
os shards estruturais por bloco e o hub de navegação já estão neste diretório.

## Propósito

Deixar inequívoco qual bloco está em curso e para onde a trilha aponta, sem
voltar a funcionar como crônica factual longa.

## Escopo

- ordem ativa oficial e bloco corrente;
- shards estruturais por bloco;
- convergência bare-metal e bootstrap.

## Fora do escopo

- o que já aconteceu → território **Histórico** (`../history/README.md`);
- inventário técnico não priorizado → `../future.md` (Engine).

## Autoridade

O Roadmap é proprietário da ordem ativa e do bloco corrente. Não é crônica
histórica nem declara implementação por interpretação.

## Mapa

| Necessidade | Documento |
|---|---|
| ordem ativa oficial | `../roadmap.md` |
| hub de navegação por blocos | `indice.md` |
| shards estruturais por bloco | `blocos/` |
| convergência bare-metal/bootstrap | `bare_metal_bootstrap.md` |

## Direção

<!-- @pinker-doc:start
id: roadmap.current
tags: [roadmap, corrente, ordem-ativa]
aliases:
  - ordem ativa
  - qual a ordem atual
summary: Onde vive a ordem ativa oficial do roadmap.
-->
### Ordem corrente

A ordem ativa oficial vive em `../roadmap.md`. O portal não duplica a ordem: ele
aponta para a fonte canônica e para os shards estruturais por bloco.
<!-- @pinker-doc:end roadmap.current -->

<!-- @pinker-doc:start
id: roadmap.active-block
tags: [roadmap, bloco, corrente]
aliases:
  - bloco corrente
  - qual bloco esta ativo
summary: Como localizar o bloco corrente e seus shards estruturais.
-->
### Bloco corrente

O bloco corrente é determinado pela ordem ativa em `../roadmap.md`; seus shards
estruturais estão em `blocos/` e a convergência bare-metal em
`bare_metal_bootstrap.md`.
<!-- @pinker-doc:end roadmap.active-block -->

<!-- @pinker-doc:start
id: roadmap.next
tags: [roadmap, proximo, direcao]
aliases:
  - proxima fase
  - qual a proxima fase
  - proxima direcao
summary: Direção imediata da trilha — para onde o roadmap aponta a seguir.
-->
### Próxima direção

Para descobrir a próxima direção da trilha, consulte a ordem ativa em
`../roadmap.md` e o hub de navegação `indice.md`. A próxima direção nunca é
inferida por heurística: ela é declarada na ordem ativa.
<!-- @pinker-doc:end roadmap.next -->

## Rotas de leitura

### Descobrir a próxima direção
1. `../roadmap.md`
2. `indice.md`

### Aprofundar um bloco específico
1. `indice.md`
2. `blocos/`

## Ciclo de vida

Um bloco entra por decisão estratégica explícita, avança por fases numeradas e
encerra por suficiência; o encerramento não apaga a crônica.

## Saídas

- **Engine:** `../engine/README.md`
- **Histórico:** `../history/README.md`
