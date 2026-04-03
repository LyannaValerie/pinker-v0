# Atlas documental da Pinker v0

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Este é o arquivo mestre de navegação documental da Pinker v0.

## 1) Hemisfério Engine (factual / operacional)

Documentos canônicos:

- `docs/roadmap.md` — ordem ativa oficial.
- `docs/history.md` — crônica histórica única (fases, hotfixes, docs e paralelas).
- `docs/agent_state.md` — estado operacional corrente, de forma enxuta.
- `docs/handoff_codex.md` — bilhete operacional curto da rodada.
- `docs/doc_rules.md` — regras de atualização documental.
- `MANUAL.md` — manual prático de uso no estado implementado.

Apoio Engine:

- `docs/future.md` — inventário técnico (não é roadmap).
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

## 4) Convenção de classe/papel/status

Quando um documento tiver papel estrutural, declarar no topo:

- **Classe:** `Engine`, `Rosa` ou `Ponte`
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

## 5) Regra de precedência resumida

1. Código mergeado local
2. `docs/roadmap.md`
3. `docs/agent_state.md` + `docs/handoff_codex.md`
4. `docs/history.md`
5. `docs/future.md`
6. `docs/rosa.md` + `docs/vocabulario.md` + `docs/parallel.md`
7. `docs/ponte_engine_rosa.md` e `docs/atlas.md`
