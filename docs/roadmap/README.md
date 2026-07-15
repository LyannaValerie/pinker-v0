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
