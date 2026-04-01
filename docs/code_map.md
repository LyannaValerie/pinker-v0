# Mapa curto de código

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Referência rápida para localizar a camada certa antes de editar.

## Frontend

- tokens e spans: `src/token.rs`
- léxico: `src/lexer.rs`
- AST: `src/ast.rs`
- parser: `src/parser.rs`

## Semântica e layout

- checagem semântica principal: `src/semantic.rs`
- layout de tipos compostos: `src/layout.rs`
- erros/renderização comum: `src/error.rs`, `src/printer.rs`

## Pipeline intermediário

- IR estruturada: `src/ir.rs`, `src/ir_validate.rs`
- CFG IR: `src/cfg_ir.rs`, `src/cfg_ir_validate.rs`
- seleção de instruções: `src/instr_select.rs`, `src/instr_select_validate.rs`
- máquina abstrata: `src/abstract_machine.rs`, `src/abstract_machine_validate.rs`

## Execução e saída

- interpretador `--run`: `src/interpreter.rs`
- subprocessos mínimos e argv explícito camada 1: `src/interpreter.rs`
- REPL mínimo `pink repl`: `src/repl.rs`
- backend textual final: `src/backend_text.rs`, `src/backend_text_validate.rs`
- backend `.s`: `src/backend_s.rs`
- boot/freestanding: `src/boot.rs`
- CLI: `src/main.rs`

## Editor/TUI

- editor oficial mínimo: `src/editor_tui.rs`
- paleta/tema: `src/palette.rs`

## Testes por camada

- frontend: `tests/lexer_tests.rs`, `tests/parser_tests.rs`
- semântica: `tests/semantic_tests.rs`
- IR/CFG/seleção: `tests/ir_tests.rs`, `tests/cfg_ir_tests.rs`, `tests/instr_select_tests.rs`
- máquina/runtime: `tests/abstract_machine_tests.rs`, `tests/abstract_machine_stack_tests.rs`, `tests/interpreter_tests.rs`
- backends: `tests/backend_text_tests.rs`, `tests/backend_s_tests.rs`, `tests/backend_s_external_toolchain_tests.rs`
- CLI/saída: `tests/output_tests.rs`, `tests/editor_tui_tests.rs`

## Docs que costumam acompanhar mudança funcional

- estado e regras: `docs/doc_rules.md`, `docs/agent_state.md`, `docs/handoff_codex.md`
- ordem e continuidade: `docs/roadmap.md`, `docs/history.md`
- uso e navegação: `README.md`, `MANUAL.md`, `docs/atlas.md`
