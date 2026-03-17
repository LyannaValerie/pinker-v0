# Handoff Codex (executor)

## Rodada atual
- Implementação da **FASE 21b — stack trace simples de runtime**.

## Objetivo
- Melhorar diagnóstico de erro de runtime com pilha simples de chamadas, sem abrir debugger/stepping.
- Preservar comportamento funcional do interpretador.

## Estado real encontrado
- Workspace local usado como fonte de verdade.
- Base inicial saudável: `cargo build` e `cargo test` passando.

## Modelagem adotada para stack trace
- `run_program` agora cria e passa uma pilha de chamadas mutável (`Vec<String>`) para o interpretador.
- `call_function` faz `push` do frame ao entrar e `pop` ao sair.
- Em erro de runtime, `call_function` anexa stack trace textual simples ao erro.
- Formato adicionado ao `msg` de runtime:
  - linha `stack trace:`
  - uma linha por frame: `  - <função>`
  - ordem da mais externa para a mais interna.
- Implementado de forma pequena via helper `attach_runtime_trace` em `src/interpreter.rs`.

## Contexto exibido
- nomes de funções ativas no momento do erro.
- sem spans por frame, sem locals por frame, sem labels (adiado para manter escopo mínimo).

## Testes adicionados/ajustados
- `tests/interpreter_tests.rs`:
  - ajuste de `run_falha_divisao_por_zero` para validar presença de `stack trace` + `principal`
  - novo `run_falha_runtime_em_chamada_tem_stack_trace`
  - novo `run_falha_runtime_em_recursao_tem_stack_trace`
  - ajuste de `cli_run_erro_runtime_tem_exit_nonzero` para validar stack trace em stderr
  - ajuste de `cli_run_erro_runtime_em_exemplo_novo` para validar stack trace em stderr

## Limites que continuam
- stack trace sem label/bloco por frame
- sem captura de variáveis locais
- sem debugger/stepping/tracing contínuo

## Estado real encontrado
- Workspace local usado como fonte de verdade.
- Base inicial saudável: `cargo build` e `cargo test` passando.

## Comandos executados
- Inicial:
  - `cargo build`
  - `cargo test`
- Final:
  - `cargo build`
  - `cargo check`
  - `cargo fmt --check`
  - `cargo test`
  - `cargo run -q -- --run examples/run_div_zero_cli.pink`

## Próximos passos sugeridos
- opcional: incluir label/bloco atual por frame (se continuar barato)
- opcional: normalizar visual do stack trace para ter índice de profundidade
