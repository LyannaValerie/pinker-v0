---
pinker-doc: 1
id: language
domain: language
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - language.territory
related:
  - rosa
  - engine
---

# Linguagem — território lexical e de famílias

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Portal do território **Linguagem**. Os documentos lexicais canônicos permanecem
na raiz de `docs/`; este portal os organiza como território.

## Propósito

Reunir a arquitetura lexical da Pinker: vocabulário, intrínsecas e famílias
temáticas, distinguindo nomeação arquitetural de implementação futura.

## Escopo

- vocabulário canônico e voz lexical;
- inventário de intrínsecas;
- famílias temáticas e sua superfície/resolução.

## Fora do escopo

- por que a voz é assim → território **Rosa** (`../rosa/README.md`);
- estado de implementação → território **Engine**.

## Autoridade

A Linguagem é proprietária da arquitetura lexical. Decisões de tom e intenção
são compartilhadas com Rosa via **Ponte**.

## Mapa

| Necessidade | Documento |
|---|---|
| arquitetura lexical canônica | `../vocabulario.md` |
| inventário de intrínsecas | `../inventario_intrinsecas.md` |
| decisão das famílias públicas | `../familias_tematicas.md` |
| domínios internos por intrínseca | `../familias/dominios.md` |
| superfície futura por família | `../familias/superficie.md` |
| resolução qualificada por família | `../familias/resolucao.md` |
| família exemplar `tempo` | `../familias/tempo.md` |

## Rotas de leitura

### Alterar ou propor vocabulário
1. `../vocabulario.md`
2. `../rosa/core.md`
3. `../bridge/engine-rosa.md`

### Entender uma família temática
1. `../familias_tematicas.md`
2. `../familias/dominios.md`

## Ciclo de vida

Decisões lexicais são humanas ou do agente; nomeação arquitetural pode preceder
implementação, mas nunca deve fingir que já existe.

## Saídas

- **Rosa:** `../rosa/README.md`
- **Ponte:** `../bridge/README.md`
- **Engine:** `../engine/README.md`
