---
pinker-doc: 1
id: development
domain: development
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - development.territory
related:
  - engine
  - language
---

# Desenvolvimento — território de apps, exemplos e navegação de código

- **Classe:** Engine
- **Papel:** navegação
- **Status:** ativo

Portal do território **Desenvolvimento**. Reúne o que orienta o trabalho prático
sobre a base: aplicações internas em Pinker, exemplos/testes e o mapa de código.

## Propósito

Dar entrada rápida a quem vai escrever código, exemplos ou apps sobre a Pinker,
sem varrer `src/` ou `examples/` indiscriminadamente.

## Escopo

- regras para aplicações internas escritas em Pinker;
- índice de exemplos e testes;
- mapa de código por feature e navegação semântica.

## Fora do escopo

- estado e roadmap → território **Engine** / **Roadmap**;
- léxico → território **Linguagem**.

## Autoridade

O Desenvolvimento é proprietário das convenções de apps internas e do índice de
exemplos. O mapa de código aponta, mas não substitui, a navegação `pink nav`.

## Mapa

| Necessidade | Documento |
|---|---|
| regras de apps internas | `../apps.md` |
| índice de exemplos e testes | `../examples_index.md` |
| mapa de código por feature | `../code_map.md` |
| navegação semântica do código | `pink nav buscar "<conceito>"` |

## Rotas de leitura

### Escrever ou alterar um app interno
1. `../apps.md`
2. `../code_map.md`

### Escolher um exemplo/teste próximo
1. `../examples_index.md`

## Ciclo de vida

Um app ou exemplo entra com propósito auditável; o mapa de código é atualizado
junto com a feature, e os marcadores `@pinker-nav` acompanham o código.

## Saídas

- **Engine:** `../engine/README.md`
- **Linguagem:** `../language/README.md`
