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
| C | publicação GitHub, corpo de PR, checks e retomada | `trama_projection_tests`, `trama_scale_tests` |
| D | fechamento formal e congelamento separado do contrato V1 | nenhuma ativação automática |

A Onda A não conclui a ferramenta e não conclui a Trama. B e C cartografam as
quatro suítes restantes. D fecha a cadeia operacional em trabalho separado.
Onda C é a próxima etapa e cobre `trama_projection_tests.rs` e
`trama_scale_tests.rs`. `trama_complete = false`; Onda 9 inativa; `apps/`
reservada. Nenhuma onda ativa automaticamente a Onda 9.
Como continuidade literal do contrato anterior, `apps/` continua reservada.
<!-- @pinker-doc:end development.pink-agent-roadmap.waves -->
