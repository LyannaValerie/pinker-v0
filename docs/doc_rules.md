# Regras de documentação da Pinker — referência obrigatória para agentes de IA

- **Classe:** Engine
- **Papel:** regra
- **Status:** ativo

## 1. Arquitetura documental oficial

A documentação da Pinker segue arquitetura dual:

- **Engine**: estado factual/operacional.
- **Rosa**: identidade lexical/estética/visionária.
- **Ponte**: documentos que conectam ambos sem confundir presente e aspiração.

Arquivo mestre de navegação: `docs/atlas.md`.

## 2. Quando atualizar cada documento

| Evento | Documento(s) | O que registrar |
|---|---|---|
| fase funcional concluída | `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada de fase, estado corrente e handoff curto |
| hotfix extraordinário | `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada HF dedicada e impacto operacional |
| rodada documental | `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada Doc dedicada e limites da rodada |
| rodada paralela de implementação | `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada Paralela dedicada |
| abertura/fechamento de bloco | `docs/roadmap.md`, `docs/agent_state.md`, `docs/history.md` | transição de bloco e justificativa |
| decisão lexical relevante | `docs/vocabulario.md`, `docs/history.md` | aceitação/rejeição/provisório + referência histórica |
| criação/mudança estrutural de docs | `docs/atlas.md`, `README.md`, `docs/history.md`, `docs/handoff_codex.md` | navegação atualizada e migração registrada |

## 3. Formato por documento

- `docs/atlas.md`: navegação mestre e classes documentais.
- `roadmap.md`: trilha ativa oficial e ordem de execução real.
- `future.md`: inventário técnico amplo; não impõe sequência ativa.
- `rosa.md`: visão identitária canônica da linguagem.
- `parallel.md`: acervo visionário de apoio (não backlog).
- `ponte_engine_rosa.md`: conexão explícita entre factual e visão.
- `history.md`: histórico único (FASES/HOTFIXES/DOCUMENTAÇÃO/RODADAS PARALELAS).
- `agent_state.md`: estado presente + diretrizes operacionais.
- `handoff_codex.md`: handoff curto da rodada atual.
- `vocabulario.md`: arquitetura lexical canônica.

## 4. Classificação obrigatória para papéis estruturais

Documentos novos (ou antigos com papel estrutural novo) devem declarar no topo, quando fizer sentido:

- **Classe:** Engine / Rosa / Ponte
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

## 5. O que NÃO colocar em cada documento

- `agent_state.md` não vira histórico extenso.
- `history.md` não vira guia operacional.
- `handoff_codex.md` não duplica crônica completa.
- `future.md` não vira roadmap.
- `parallel.md` não vira backlog técnico executável.
- `rosa.md` não declara funcionalidades como implementadas sem evidência Engine.

## 6. Precedência documental

1. Código mergeado.
2. `roadmap.md` (ordem ativa).
3. `agent_state.md` + `handoff_codex.md` (estado operacional da rodada).
4. `history.md` (continuidade factual).
5. `future.md` (inventário técnico).
6. `rosa.md` + `vocabulario.md` + `parallel.md` (identidade e visão).
7. `ponte_engine_rosa.md` e `atlas.md` (mediação e navegação).

## 7. Tom e linguagem

- português;
- objetivo;
- autocontido;
- sem inflar retórica;
- sem duplicação desnecessária.

