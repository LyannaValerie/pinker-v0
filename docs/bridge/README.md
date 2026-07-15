---
pinker-doc: 1
id: bridge
domain: bridge
kind: portal
status: active
parent: atlas
audience:
  - human
  - agent
canonical_for:
  - bridge.territory
related:
  - bridge.engine-rosa
  - rosa
---

# Ponte — mediação Engine ↔ Rosa

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Portal do território **Ponte**. Ele media o estado factual (Engine), a direção
identitária (Rosa) e a agência executável determinística (Guardião Pinker).

## Propósito

Conectar explicitamente hemisférios que não devem se confundir nem se ignorar:
o que está pronto hoje, para onde o projeto quer permanecer coerente, e o que um
contrato executável realmente verifica.

## Escopo

- regras de conversa entre Engine e Rosa;
- decisão prática por tipo de tema;
- gate de consistência antes de encerrar rodada;
- regra de conflito quando os hemisférios divergem.

## Fora do escopo

- identidade e voz em si → território **Rosa** (`../rosa/README.md`);
- estado, fases e pipeline → território **Engine**;
- implementação do Guardião → `../../apps/guardiao_pinker/`.

## Autoridade

A Ponte governa o protocolo de mediação e o gate de consistência entre
hemisférios. Não é proprietária de fatos técnicos (Engine) nem de invariantes
identitárias (Rosa); ela os relaciona sem falsear nenhum dos dois.

## Mapa

| Necessidade | Documento |
|---|---|
| mediação Engine ↔ Rosa | `engine-rosa.md` |
| identidade e voz | `../rosa/README.md` |
| léxico canônico | `../vocabulario.md` |

## Rotas de leitura

### Decidir a quem pertence um tema
1. `engine-rosa.md` (seção 4)

### Fechar rodada sem incoerência
1. `engine-rosa.md` (seção 5)
2. `../rosa/voice-tests.md`

## Ciclo de vida

As regras de mediação mudam por decisão explícita e versionada; conflitos
resolvidos viram direção registrada, nunca apagamento de restrição técnica.

## Saídas

- **Rosa:** `../rosa/README.md`
- **Linguagem:** `../vocabulario.md`
