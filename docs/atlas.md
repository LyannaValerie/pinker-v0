# Atlas documental da Pinker v0

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Este é o arquivo mestre de navegação documental da Pinker v0.

## 1) Hemisfério Engine (factual / operacional)

Documentos canônicos:

- `docs/roadmap.md` — ordem ativa oficial.
- `docs/roadmap/indice.md` — hub de navegação do roadmap por blocos.
- `docs/roadmap/blocos/*.md` — shards estruturais por bloco; não substituem o histórico.
- `docs/history.md` — ponteiro canônico curto para o sistema histórico.
- `docs/history/indice.md` — hub principal de navegação histórica.
- `docs/agent_state.md` — estado operacional corrente, de forma enxuta.
- `docs/handoff_codex.md` — bilhete operacional curto da rodada.
- `docs/doc_rules.md` — regras de atualização documental.
- `MANUAL.md` — manual prático de uso no estado implementado.

Apoio Engine:

- `docs/history/phases/indice.md` — índice local das fases históricas.
- `docs/history/hotfixes/indice.md` — índice local dos hotfixes.
- `docs/history/documentation/indice.md` — índice local das rodadas documentais.
- `docs/history/parallel_phases/indice.md` — índice local das rodadas paralelas.
- `docs/future.md` — inventário técnico (não é roadmap).
- `docs/inventario_intrinsecas.md` — inventário canônico de intrínsecas (Bloco 18, Fase 180).
- `docs/phases.md` — arquivo de compatibilidade para referências legadas.

## 2) Hemisfério Rosa (identitário / lexical / visão)

Documentos canônicos:

- `docs/rosa.md` — identidade/visão da linguagem.
- `docs/vocabulario.md` — arquitetura lexical canônica.

Apoio Rosa:

- `docs/parallel.md` — acervo visionário (não backlog técnico).

## 3) Ponte explícita Engine ↔ Rosa

- `docs/ponte_engine_rosa.md` — regras de mediação entre estado factual e direção identitária.
- `docs/familias_tematicas.md` — decisão canônica inicial das famílias públicas do Bloco 18, distinguindo nomeação arquitetural de implementação futura.
- `docs/familias/tempo.md` — formalização curta da família exemplar `tempo` no Bloco 18.

## 4) Convenção de classe/papel/status

Quando um documento tiver papel estrutural, declarar no topo:

- **Classe:** `Engine`, `Rosa` ou `Ponte`
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

## 5) Regra de precedência resumida

1. Código mergeado local
2. `docs/roadmap.md`
3. `docs/agent_state.md` + `docs/handoff_codex.md`
4. sistema histórico canônico (`docs/history.md` -> `docs/history/indice.md` -> shards em `docs/history/`)
5. `docs/future.md`
6. `docs/rosa.md` + `docs/vocabulario.md` + `docs/parallel.md`
7. `docs/ponte_engine_rosa.md` e `docs/atlas.md`
