# Atlas documental da Pinker v0

- **Classe:** Ponte
- **Papel:** navegação
- **Status:** ativo

Este é o arquivo mestre de navegação documental da Pinker v0.

## Tese estrutural desta arquitetura

A documentação da Pinker está organizada em dois hemisférios complementares:

- **Hemisfério Engine (factual/operacional):** o que já existe, como funciona hoje, como validar e como evoluir sem perder continuidade.
- **Hemisfério Rosa (identitário/visionário):** a voz da linguagem, critérios lexicais, visão de ecossistema e direção conceitual de longo prazo.

A documentação só está correta quando esses hemisférios conseguem conversar sem confundir fato com ambição.

---

## 1) Hemisfério Engine (factual / operacional)

Documentos canônicos para estado real e continuidade técnica:

- `docs/roadmap.md` — trilha ativa oficial e estado de bloco corrente (incluindo encerramentos conservadores).
- `docs/history.md` — linha do tempo factual (fases, hotfixes, docs e paralelas).
- `docs/agent_state.md` — estado operacional corrente e diretrizes de execução.
- `docs/handoff_codex.md` — handoff curto da rodada vigente.
- `docs/doc_rules.md` — regras obrigatórias de atualização documental.
- `MANUAL.md` — manual prático de uso da linguagem no estado implementado.

Documento de apoio do hemisfério Engine:

- `docs/future.md` — inventário técnico amplo (não ordena a trilha ativa).

---

## 2) Hemisfério Rosa (identitário / lexical / visão)

Documentos canônicos para identidade, léxico e visão de linguagem viva:

- `docs/rosa.md` — manifesto conceitual estruturado da Pinker/Rosa.
- `docs/vocabulario.md` — arquitetura lexical canônica (critérios, famílias, aceitas/rejeitadas/provisórias).

Documento de apoio do hemisfério Rosa:

- `docs/parallel.md` — repositório de material visionário legado e notas de origem conceitual (não backlog).

---

## 3) Ponte explícita Engine ↔ Rosa

- `docs/ponte_engine_rosa.md` — documento-ponte com mapeamento entre estado real e direção identitária.

Este arquivo existe para impedir dois desvios simétricos:

1. engenharia sem identidade;
2. identidade sem chão operacional.

---

## 4) Convenção de classe/papel/status

Sempre que um documento ganhar papel estrutural, declarar no topo:

- **Classe:** `Engine`, `Rosa` ou `Ponte`
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

---

## 5) Regra de precedência resumida

1. Código mergeado local
2. `docs/roadmap.md` (ordem ativa)
3. `docs/agent_state.md` + `docs/handoff_codex.md` (estado da rodada)
4. `docs/history.md` (crônica factual)
5. `docs/future.md` (inventário técnico)
6. `docs/rosa.md` + `docs/vocabulario.md` + `docs/parallel.md` (identidade e visão)
