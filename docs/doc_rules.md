# Regras de documentação da Pinker — referência obrigatória para agentes de IA

## 1. Quando atualizar cada documento

| Evento | Documento(s) | O que registrar |
|---|---|---|
| fase funcional concluída | `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada de fase em `phases`, estado corrente atualizado, handoff curto da rodada |
| hotfix extraordinário | `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada HF dedicada, impacto operacional e continuidade da trilha |
| rodada documental | `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md` | entrada Doc dedicada, escopo documental e limites da rodada |
| abertura de bloco | `docs/roadmap.md`, `docs/agent_state.md`, `docs/phases.md` | bloco ativo, justificativa e ponto de transição registrado |
| fechamento de bloco | `docs/roadmap.md`, `docs/agent_state.md`, `docs/phases.md` | confirmação de fechamento e transição para próximo bloco |
| keyword implementada | `docs/vocabulario.md`, `docs/phases.md` | mover keyword para implementadas + referência da fase |
| item de future implementado | `docs/future.md`, `docs/phases.md` | marcar como implementado e referenciar fase |
| item de future parcialmente implementado | `docs/future.md`, `docs/phases.md` | marcar parcial (🔶) e limitar escopo real entregue |
| criação de documento novo | `README.md`, `docs/phases.md`, `docs/handoff_codex.md` | incluir no ecossistema documental e registrar propósito |

## 2. Formato por documento
- `roadmap.md`: trilha ativa oficial e ordem de execução real.
- `future.md`: inventário técnico amplo de possibilidades/status; sem impor sequência ativa.
- `parallel.md`: visão orientadora, identidade e direção conceitual; não técnico-operacional.
- `phases.md`: histórico único em três blocos fixos (FASES/HOTFIXES/DOCUMENTAÇÃO), entradas autocontidas.
- `agent_state.md`: estado presente + diretrizes operacionais; sem histórico extenso.
- `handoff_codex.md`: handoff curto da rodada atual (objetivo, escopo, próximo passo).
- `vocabulario.md`: catálogo vivo de keywords; implementadas separadas de sugestões.

## 3. O que NÃO colocar em cada documento
- `agent_state.md` não acumula histórico completo de fases.
- `phases.md` não acumula diretrizes operacionais de agente.
- `handoff_codex.md` não duplica histórico completo do projeto.
- `future.md` não dita ordem ativa de execução.
- `parallel.md` não cria fases nem backlog técnico executável.
- `roadmap.md` não vira backlog amplo e difuso.

## 4. Precedência documental
- Código mergeado prevalece sobre documentação.
- `roadmap.md` decide a trilha ativa.
- `future.md` organiza backlog/inventário amplo.
- `parallel.md` guarda a visão orientadora.
- `phases.md` guarda o histórico.
- `agent_state.md` guarda o estado corrente.
- `handoff_codex.md` guarda o handoff operacional curto.
- `doc_rules.md` guarda as convenções documentais.

## 5. Tom e linguagem
- português.
- objetivo.
- autocontido.
- sem inflação retórica.
- sem duplicação desnecessária.

## 6. Convenção visual obrigatória
- Fases construtivas em seção **FASES**.
- Hotfixes em seção **HOTFIXES**.
- Documentação em seção **DOCUMENTAÇÃO**.
- Numeração independente por categoria.
- Exemplos formais:
  - `55 - assembler/linker externo (integração mínima)`
  - `HF-1 - hotfixes de corretude e manutenção`
  - `Doc-1 - abertura documental do Bloco 6`
