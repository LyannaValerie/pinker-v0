---
pinker-doc: 1
id: engine.state
domain: engine
kind: reference
status: active
parent: engine
audience:
  - human
  - agent
related:
  - engine
  - roadmap
---

# Estado operacional da Engine

- **Classe:** Engine
- **Papel:** estado factual corrente
- **Status:** ativo

Este documento dá território consultável ao estado corrente da Engine. O texto
humano abaixo descreve o estado factual; a região gerada projeta, de forma
mecânica, o que os manifestos versionados registram após o marco #330.

<!-- @pinker-doc:start
id: engine.state.current
tags: [engine, estado, corrente, pipeline]
aliases:
  - estado atual
  - estado corrente
  - qual e o estado atual
summary: Estado factual corrente da Engine (compilador, runtime e backend implementados).
-->
## Estado corrente

A Engine implementa o pipeline completo do compilador Pinker v0: léxico,
parsing, semântica, IR, CFG, seleção de instruções, máquina abstrata, backend
textual `.s` e interpretador. O caminho oficial de validação é `make ci`.

O estado corrente **factual** é propriedade deste território; a trilha ativa e a
próxima direção pertencem ao Roadmap, e a crônica do que já aconteceu pertence
ao Histórico. Este documento não reescreve nem inventa continuidade.
<!-- @pinker-doc:end engine.state.current -->

<!-- @pinker-doc:start
id: engine.state.limits
tags: [engine, limites, restricoes]
aliases:
  - limites atuais
  - restricoes da engine
summary: Limites honestos do estado corrente da Engine.
-->
## Limites

- O backend nativo real depende de driver C do sistema e do runtime
  `libpinker_rt.a`; a ABI textual mínima interna ainda não é ABI de plataforma.
- A Trama Pinker é forward-only: nenhum estado é reconstruído retroativamente a
  partir de PRs anteriores ao marco #330.
- Este documento não afirma implementação sem evidência em código e testes.
<!-- @pinker-doc:end engine.state.limits -->

## Projeção mecânica dos manifestos

Conteúdo abaixo é **propriedade da ferramenta** (projeção `state`); não edite à
mão. Regenere com `pink doc sincronizar`.

<!-- @pinker-generated:start engine.state.generated -->
- Manifestos processados: 2
- Última mudança: PR #382 — Adiciona mapa agrupado do catálogo de código (fase —, bloco —)
- Seções implementadas: —
<!-- @pinker-generated:end engine.state.generated -->
