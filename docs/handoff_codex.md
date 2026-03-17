# Handoff Codex (executor)

## Rodada atual
- Implementação da **FASE 20 — mais testes end-to-end com `--run`**.

## Objetivo
- Aumentar confiança prática do fluxo completo da CLI real (`arquivo .pink -> parser -> semântica -> IR -> CFG -> seleção -> Machine -> interpretador`).
- Cobrir combinações realistas sem alterar linguagem/runtime.

## Estado real encontrado
- Workspace local usado como fonte de verdade.
- Fase 19 estava concluída e base inicial estava saudável (`cargo build`/`cargo test` passando).

## Cenários novos cobertos (end-to-end via CLI `--run`)
- **global + chamada**: `examples/run_global_call_combo.pink` → `42`
- **mutação local + if/else**: `examples/run_mut_if_else.pink` → `42`
- **recursão + global**: `examples/run_recursao_global.pink` → `5`
- **erro de runtime via CLI**: `examples/run_div_zero_cli.pink` → exit code não-zero e stderr com divisão por zero

## Exemplos adicionados
- `examples/run_global_call_combo.pink`
- `examples/run_mut_if_else.pink`
- `examples/run_recursao_global.pink`
- `examples/run_div_zero_cli.pink`

## Testes adicionados
- Em `tests/interpreter_tests.rs`:
  - `cli_run_mantem_exemplos_base`
  - `cli_run_global_com_chamada_exemplo_novo`
  - `cli_run_mutacao_com_if_else_exemplo_novo`
  - `cli_run_recursao_com_global_exemplo_novo`
  - `cli_run_erro_runtime_em_exemplo_novo`

## Observação sobre exemplos legados solicitados
- `examples/run_recursao_fatorial.pink` não existe neste workspace; o teste trata esse caso de forma condicional sem falha espúria.
- `examples/run_fatorial.pink` existe e foi executado nos comandos desta rodada.

## Limites que continuam
- Sem escrita em globals.
- Sem I/O da linguagem.
- Sem novas capacidades de runtime fora do escopo.

## Arquivos alterados
- `examples/run_global_call_combo.pink`
- `examples/run_mut_if_else.pink`
- `examples/run_recursao_global.pink`
- `examples/run_div_zero_cli.pink`
- `tests/interpreter_tests.rs`
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Comandos executados
- Inicial:
  - `cargo build`
  - `cargo test`
- Final:
  - `cargo build`
  - `cargo check`
  - `cargo fmt --check`
  - `cargo test`
  - `cargo run -q -- --run examples/run_soma.pink`
  - `cargo run -q -- --run examples/run_chamada.pink`
  - `cargo run -q -- --run examples/run_global.pink`
  - `cargo run -q -- --run examples/run_global_expr.pink`
  - `cargo run -q -- --run examples/run_fatorial.pink`
  - `cargo run -q -- --run examples/run_global_call_combo.pink`
  - `cargo run -q -- --run examples/run_mut_if_else.pink`
  - `cargo run -q -- --run examples/run_recursao_global.pink`
  - `cargo run -q -- --run examples/run_div_zero_cli.pink`

## Próximos passos sugeridos
- Expandir golden set de `--run` para cobrir mais casos de erro semântico observável pela CLI.
- Opcional: suíte de exemplos negativos de parse/semântica dedicada para contratos de stderr.
