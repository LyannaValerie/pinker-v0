# Handoff Codex (executor)

## Rodada atual
- **Fase 37 implementada**: contextualização dos comentários de `--machine` com heurísticas simples por alvo/slot, sem alterar semântica.

## Estado real encontrado
- Continuidade histórica validada: 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 → 31 → 32 → 33 → 34 → 35 → 36.
- Workspace local mantido como fonte de verdade.
- Baseline antes das mudanças: `cargo build` e `cargo test` passavam.

## Ação aplicada (Fase 37)
- `src/abstract_machine.rs`:
  - `store_slot` agora diferencia comentário para temporário (`%tN`) vs variável local do usuário.
  - `call` e `call_void` passaram a citar nome da função e aridade de forma direta.
  - `br_true` ganhou comentários contextuais por padrão de labels (if, loop, short-circuit).
  - `jmp` ganhou comentário contextual por alvo (`loop_cond`, `loop_join`, `join`/`logic_join`).
  - `ret` e `ret_void` receberam texto mais específico e estável.
- `tests/abstract_machine_tests.rs`:
  - snapshots/exatos atualizados para os novos comentários contextuais.
  - cobertura explícita adicionada para `call_void` e para `br_true` em loop.
  - checks de legibilidade da Fase 35 (params/locals/temps/blocos) preservados.
- Docs atualizados: `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md`.

## O que permaneceu igual
- Sem alteração na estrutura/semântica da Machine.
- Sem alteração em parser, semântica, lowering CFG, interpretador.
- Sem alteração em `--selected`, `--cfg-ir`, `--pseudo-asm`, `--run`.
- Sem criação de nova flag.

## Arquivos alterados
- `src/abstract_machine.rs`
- `tests/abstract_machine_tests.rs`
- `docs/phases.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`
- `docs/handoff_auditor.md`

## Comandos executados
- `cargo build`: ok
- `cargo check`: ok
- `cargo fmt --check`: ok
- `cargo test`: ok
- `cargo run -- --machine examples/showcase_completo.pink`: ok, com comentários contextuais melhores em `br_true`, `jmp`, `store_slot`, `call` e `call_void`.
