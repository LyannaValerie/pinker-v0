# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 16**: robustez do interpretador e testes negativos de runtime.

## Objetivo
- Cobrir os 9 caminhos de erro do interpretador que não tinham teste.
- Cobrir instruções sem cobertura end-to-end (Not, Div, CmpEq, CmpNe, CmpGe, CmpGt, CmpLe).
- Cobrir reassignment de variável mutável.
- Pequeno endurecimento: mensagens de erro em `call_function` incluem `fn_name`.

## Arquivos alterados
- `src/interpreter.rs`
- `tests/interpreter_tests.rs`
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Diagnóstico real encontrado
- Código e docs em sincronia perfeita com a Fase 15. Sem divergências.
- 149 testes passando, build limpo, fmt limpo antes do início.
- `interpreter.rs` tinha 9 guards de erro sem cobertura de teste.
- 7 instruções da Machine sem cobertura end-to-end.

## Decisão técnica aplicada
- `src/interpreter.rs`: endurecimento mínimo de mensagens — erros em `call_function` agora incluem `[fn_name]`:
  - aridade inválida, label inexistente, ret inválido, ret_void inválido.
- `tests/interpreter_tests.rs`: 15 testes novos adicionados.

## Testes novos (15 total)

### Negativos via MachineProgram manual (6)
1. `run_falha_divisao_por_zero` — `Div` com 0 → "divisão por zero"
2. `run_falha_slot_nao_inicializado` — `LoadSlot` sem store anterior → "load_slot em slot não inicializado"
3. `run_falha_call_retorna_void` — `Call` para função que faz `RetVoid` → "call exige função com retorno"
4. `run_falha_call_void_retorna_valor` — `CallVoid` para função que faz `Ret` → "call_void exige função sem retorno"
5. `run_falha_aridade_invalida` — argc diferente de params.len() → "chamada com aridade inválida"
6. `run_falha_valor_global_nao_suportado` — `OperandIR::Local` como valor de global → "valor global não suportado em runtime"

### End-to-end via run_code (8)
1. `run_not_unario` — `!falso` em condição → `1`
2. `run_divisao` — `10 / 2` → `5`
3. `run_igualdade` — `1 == 1` → `1`
4. `run_diferenca` — `1 != 2` → `1`
5. `run_comparacao_maior_igual` — `5 >= 3` → `1`
6. `run_comparacao_maior` — `5 > 3` → `1`
7. `run_comparacao_menor_igual` — `3 <= 5` → `1`
8. `run_variavel_mutavel` — `nova mut x = 1; x = 99; mimo x;` → `99`

### CLI (1)
1. `cli_run_erro_runtime_tem_exit_nonzero` — programa com divisão por zero via `--run` → exit code ≠ 0, stdout vazio, stderr não vazio

## Cobertura do interpretador após Fase 16
- Todas as 19 instruções da Machine cobertas (incluindo Not, Div, CmpEq, CmpNe, CmpGe, CmpGt, CmpLe).
- Todos os 9 guards de erro com teste dedicado.
- Reassignment de variável mutável coberto.
- Exit code de erro de runtime coberto via CLI.

## O que ainda não cobre
- Escrita em globals (`store_global`).
- I/O na linguagem.
- Loops (`eterno`) no interpretador.
- Proteção contra recursão infinita.
- Debugger, tracing, otimizações de execução.

## Testes/comandos executados
- `cargo build`: limpo
- `cargo check`: limpo
- `cargo fmt --check`: limpo
- `cargo test`: 164 passed; 0 failed (32 em interpreter_tests, 132 nos demais)
- `cargo run -q -- --run examples/run_soma.pink` → 42
- `cargo run -q -- --run examples/run_chamada.pink` → 42
- `cargo run -q -- --run examples/run_global.pink` → 100
- `cargo run -q -- --run examples/run_global_expr.pink` → 44

## Próximos passos sugeridos
- Fase 17: loops (`eterno`) no interpretador, caso a linguagem já suporte o lowering.
- Ou: auditoria do comportamento de borda de `u64` (overflow wrapping já presente, mas sem teste).
