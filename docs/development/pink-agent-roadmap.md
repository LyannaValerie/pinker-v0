---
pinker-doc: 1
id: development.pink-agent-roadmap
domain: development
kind: roadmap
status: active
parent: development
audience:
  - human
  - agent
related:
  - development.pink-agent
---

# Roadmap de `pink agente`

<!-- @pinker-doc:start
id: development.pink-agent-roadmap.waves
tags: [agente, roadmap, ondas, trama]
aliases:
  - ondas pink agente
summary: Decomposição explícita das Ondas A–D e seus limites de conclusão.
-->
## Ondas

| Onda | Entrega | Dogfood Trama |
|---|---|---|
| A completa | núcleo local, spec estrita, processos estruturados, estado, artefatos e limitações | `trama_ci_tests`, `trama_template_tests` |
| B completa | verificadores Git, marker-only, projeções e sensibilidade reversível | `trama_manifest_tests`, `trama_sync_tests` |
| C completa | publicação GitHub, corpo de PR, checks e retomada idempotente | `trama_projection_tests`, `trama_scale_tests` |
| D completa | congelamento separado do contrato V1 e corpo humano substantivo de PR | nenhuma ativação automática |

A Onda A não conclui a ferramenta e não conclui a Trama. B e C cartografam as
quatro suítes restantes; com A, são seis suítes operacionais cartografadas. A
Onda C completa entrega validação de PR body, commit/push/PR, checks do
candidato, dogfood e retomada. A Onda D completa congela o contrato
`pink-agent-v1` e exige corpo humano substantivo nas PRs publicadas pelo agente:
`pink_agent_v1_frozen = true` e `pink_agent_series_a_d_complete = true`.

As Ondas A–D pertencem exclusivamente ao `pink agente`; **não** são as Ondas
9–12 da Trama. A Onda D **não** fecha a Trama: `trama_complete = false`;
Onda 9 inativa; `apps/` reservada. A Trama continua evoluindo em paralelo e esta
onda não ativa, desativa nem implementa a Onda 9, não toca `apps/` e apenas
consome `pink nav mapa`, que já pertence à base pela PR #382. Nenhuma onda ativa
automaticamente a Onda 9. Como continuidade literal do contrato anterior,
`apps/` continua reservada. O único próximo passo é o merge manual da Founder.
<!-- @pinker-doc:end development.pink-agent-roadmap.waves -->
