# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 13**: primeiro interpretador mínimo da Pinker sobre `MachineProgram` já validada.

## Objetivo
- Executar `principal` via nova flag `--run` sem alterar frontend/gramática.
- Entregar execução mínima controlada com escopo explícito e erros curtos para recursos fora de escopo.

## Arquivos alterados
- `src/interpreter.rs`
- `src/error.rs`
- `src/lib.rs`
- `src/main.rs`
- `tests/interpreter_tests.rs`
- `examples/run_soma.pink`
- `README.md`
- `docs/handoff_codex.md`
- `docs/phases.md`

## Diagnóstico real encontrado
- O repositório estava estável após Fase 12 (build e testes passando).
- Pipeline de lowering/validação até Machine já estava consolidada e foi reaproveitada para `--run`.

## Decisão técnica aplicada
- Interpretador pequeno e direto em `src/interpreter.rs`, com frame local (`slots` + `stack`) e execução por labels.
- Nova variante mínima de erro `PinkerError::Runtime` para mensagens de runtime curtas e consistentes.
- Integração de `--run` na CLI reutilizando a pipeline existente até Machine validada.

## Cobertura atual do interpretador
- Suporta: `PushInt`, `PushBool`, `LoadSlot`, `StoreSlot`, `Neg`, `Not`, `Add/Sub/Mul/Div`, comparações numéricas, `Jmp`, `BrTrue`, `Ret`, `RetVoid`.
- Executa apenas `principal` como única função.

## Fora de escopo nesta fase (falha explícita)
- `Call`, `CallVoid`.
- globals.
- execução multi-função.
- I/O na linguagem.

## Testes/comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`
- `cargo run -q -- --run examples/run_soma.pink`

## Resultado real
- Todos os comandos passaram.
- `--run examples/run_soma.pink` imprime `42`.

## Próximos passos sugeridos
- Expandir execução para chamadas entre funções (com stack frame por chamada), mantendo disciplina mínima.
- Definir política explícita para overflow aritmético/semântica de inteiros no runtime.
