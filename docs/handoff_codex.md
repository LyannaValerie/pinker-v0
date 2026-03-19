# Handoff Codex (executor)

## Rodada atual
- **Fase 38 implementada**: comentários de `--machine` ficaram sensíveis ao papel do fluxo (if/loop/curto-circuito/join), sem alterar semântica.

## Estado real encontrado
- Continuidade histórica validada: 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 → 31 → 32 → 33 → 34 → 35 → 36 → 37.
- Workspace local mantido como fonte de verdade.
- Baseline antes das mudanças: `cargo build` e `cargo test` passavam.

## Ação aplicada (Fase 38)
- `src/abstract_machine.rs`:
  - `render_term` passou a receber o bloco atual para comentar com contexto de fluxo.
  - `br_true` refinado para diferenciar melhor `talvez/senao`, `sempre que` e curto-circuito lógico.
  - `jmp` refinado por alvo (`loop_cond_*`, `loop_join_*`, `join_*`, `logic_join_*`, `loop_break_cont_*`, `loop_continue_cont_*`) e por contexto de bloco atual.
  - anotações de bloco atualizadas para `join_*` e `logic_join_*` com foco em retomada de fluxo.
  - suporte de anotação para `loop_break_cont_*` e `loop_continue_cont_*` quando aparecerem.
- `tests/abstract_machine_tests.rs`:
  - expectativas de comentários de `br_true` (if/loop) atualizadas para o novo texto contextual.
  - novos testes de substring para curto-circuito (`br_true`) e para `jmp` em `join_*`/`logic_join_*`.
  - checks de legibilidade herdados das Fases 35/36/37 preservados.
- Docs atualizados: `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/handoff_auditor.md`.

## O que permaneceu igual
- Sem alteração na semântica/estrutura da Machine.
- Sem alteração em parser, semântica, lowering CFG, interpretador.
- Sem alteração em `--selected`, `--cfg-ir`, `--pseudo-asm`, `--run`.
- Sem criação de flag nova.

## Arquivos alterados
- `src/abstract_machine.rs`
- `tests/abstract_machine_tests.rs`
- `docs/phases.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`
- `docs/handoff_auditor.md`
