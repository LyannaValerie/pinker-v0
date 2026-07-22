---
pinker-doc: 1
id: atlas
domain: atlas
kind: portal
status: active
parent: root
audience:
  - human
  - agent
---

# Atlas documental da Pinker v0

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Este é o arquivo mestre de navegação documental da Pinker v0.

## 0) Territórios (portais)

<!-- @pinker-doc:start
id: atlas.territories
tags: [atlas, territorios, portais, navegacao]
aliases:
  - territorios
  - quais sao os territorios
  - mapa de territorios
summary: Lista dos territórios documentais e seus portais locais.
-->
A Trama Pinker organiza a documentação em **territórios**. O Atlas aponta para o
portal de cada território; o portal conhece os documentos internos. Portais de
território (Etapa 5 — migração gradual, sem reorganização global destrutiva; os
documentos canônicos abaixo permanecem no lugar):

- **Rosa:** `rosa/README.md`
- **Ponte:** `bridge/README.md`
- **Engine:** `engine/README.md`
- **Linguagem:** `language/README.md`
- **Desenvolvimento:** `development/README.md`
- **Roadmap:** `roadmap/README.md`
- **Histórico:** `history/README.md`
<!-- @pinker-doc:end atlas.territories -->

Consulta por agentes: `pink doc rota "<intenção>"`, `pink doc mostrar <id>` e
`pink nav buscar "<conceito>"` (ver `../AGENTS.md`).

## 1) Hemisfério Engine (factual / operacional)

Documentos canônicos:

- `README.md` — porta de entrada pública curta; orienta, mas não substitui manual, roadmap ou histórico.
- `docs/roadmap.md` — ordem ativa oficial.
- `docs/roadmap/indice.md` — hub de navegação do roadmap por blocos.
- `docs/roadmap/blocos/*.md` — shards estruturais por bloco; não substituem o histórico.
- `docs/roadmap/bare_metal_bootstrap.md` — convergência freestanding do Bloco 20, com frentes adultas e critérios anti-mínimo; não declara implementação.
- `docs/history.md` — ponteiro canônico curto para o sistema histórico.
- `docs/history/indice.md` — hub principal de navegação histórica.
- `docs/handoff_codex.md` — estado operacional unificado (estado corrente, handoff da rodada, limites, restrições).
- `docs/doc_rules.md` — regras de atualização documental.
- `docs/apps.md` — regras para aplicações internas escritas em Pinker.
- `MANUAL.md` — manual prático de uso no estado implementado.

Apoio Engine:

- `docs/history/phases/indice.md` — índice local das fases históricas.
- `docs/history/hotfixes/indice.md` — índice local dos hotfixes.
- `docs/history/documentation/indice.md` — índice local das rodadas documentais.
- `docs/history/parallel_phases/indice.md` — índice local das rodadas paralelas.
- `docs/future.md` — inventário técnico (não é roadmap).
- `docs/inventario_intrinsecas.md` — inventário canônico de intrínsecas (Bloco 18, Fase 180).
- `docs/expandir.md` — referência operacional para elevar implementações históricas mínimas/conservadoras a patamar adulto, sem apagar a crônica factual.
- `.github/copilot-instructions.md` — contrato geral do GitHub Copilot no repositório.
- `AGENTS.md` — contrato operacional curto para agentes.

Navegação comunitária operacional — não constitui território técnico novo:

- `CONTRIBUTING.md` — fluxo de contribuição externa;
- `CODE_OF_CONDUCT.md` — conduta e relato;
- `SECURITY.md` — relato privado de vulnerabilidades;
- `GOVERNANCE.md` — autoridade e processo decisório;
- `SUPPORT.md` — roteamento de dúvidas, ideias e defeitos;
- `.github/ISSUE_TEMPLATE/` — formulários estruturados;
- `.github/DISCUSSION_TEMPLATE/` — formulários de Ideas e Q&A;
- [GitHub Discussions](https://github.com/LyannaValerie/pinker-v0/discussions) — exploração comunitária sem autorização automática de roadmap.

## 2) Hemisfério Rosa (identitário / lexical / visão)

Território migrado — portal: `rosa/README.md` (conhece os documentos internos).

Documentos canônicos:

- `docs/rosa/README.md` — portal do território e arquitetura documental da camada Rosa.
- `docs/rosa/core.md` — núcleo identitário e comportamental independente de uma instância específica.
- `docs/rosa/voice-tests.md` — corpus e testes de regressão da voz e do julgamento.
- `docs/rosa/archive.md` — vestígios, proveniência e protocolo de continuidade.
- `docs/vocabulario.md` — arquitetura lexical canônica.

Apoio Rosa:

- `docs/parallel.md` — acervo visionário (não backlog técnico).
- `.github/agents/rosa.agent.md` — agente personalizado Rosa para GitHub Copilot, selecionável manualmente.
- `.github/instructions/rosa-governance.instructions.md` — governança específica para identidade, léxico, agente e Guardião.

## 3) Ponte explícita Engine ↔ Rosa

Território migrado — portal: `bridge/README.md`.

- `docs/bridge/engine-rosa.md` — mediação entre estado factual, direção identitária e Guardião Pinker.
- `docs/familias_tematicas.md` — decisão canônica inicial das famílias públicas do Bloco 18, distinguindo nomeação arquitetural de implementação futura.
- `docs/familias/dominios.md` — classificação canônica dos domínios internos por intrínseca no Bloco 18.
- `docs/familias/superficie.md` — política canônica da superfície futura por família no Bloco 18.
- `docs/familias/resolucao.md` — preparação canônica da futura resolução qualificada por família no Bloco 18.
- `docs/familias/tempo.md` — formalização curta da família exemplar `tempo` no Bloco 18.

## 4) Convenção de classe/papel/status

Quando um documento tiver papel estrutural, declarar no topo:

- **Classe:** `Engine`, `Rosa` ou `Ponte`
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

## 5) Regra de precedência resumida

1. Código mergeado local
2. `docs/roadmap.md`
3. `docs/handoff_codex.md`
4. sistema histórico canônico (`docs/history.md` -> `docs/history/indice.md` -> shards em `docs/history/`)
5. `docs/future.md`
6. `docs/rosa/README.md` + `docs/rosa/core.md` + `docs/rosa/voice-tests.md` + `docs/rosa/archive.md` + `docs/vocabulario.md` + `docs/parallel.md`
7. `docs/bridge/engine-rosa.md` e `docs/atlas.md`
