# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 15**: suporte a globals no interpretador da Machine.

## Objetivo
- Remover limitação de `load_global` no runtime com implementação mínima e auditável.
- Manter globals como somente leitura.

## Arquivos alterados
- `src/interpreter.rs`
- `tests/interpreter_tests.rs`
- `examples/run_global.pink`
- `examples/run_global_expr.pink`
- `README.md`
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Diagnóstico real encontrado
- O interpretador da Fase 14 já suportava chamadas entre funções (`call`/`call_void`), mas ainda falhava para `load_global`.
- `MachineProgram.globals` usa `MachineGlobal { name, value }` e `value` é `OperandIR`.

## Decisão técnica aplicada
- Construído mapa de globals uma vez por execução (`build_globals`).
- Avaliação mínima de valores de globals em runtime:
  - `OperandIR::Int` → `RuntimeValue::Int`
  - `OperandIR::Bool` → `RuntimeValue::Bool`
- `load_global` agora faz lookup no mapa e empilha cópia do valor.

## Política de globals nesta fase
- Globals são somente leitura.
- Se global não existir: erro curto (`global inexistente em runtime`).
- Se valor da global não for literal suportado: erro curto (`valor global não suportado em runtime`).

## Cobertura atual do interpretador
- Tudo da Fase 14 (controle de fluxo, slots, aritmética, comparações, chamadas entre funções).
- Agora também leitura de globals no runtime via `load_global`.

## O que ainda não cobre
- Escrita em globals.
- I/O na linguagem.
- infraestrutura avançada de runtime (debugging/tracing/otimizações de execução).

## Limites remanescentes
- Sem mutação de globals.
- Sem avaliação de formatos complexos de valores globais além dos literais já usados pelo lowering atual.

## Testes/comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`
- `cargo run -q -- --run examples/run_global.pink`
- `cargo run -q -- --run examples/run_global_expr.pink`
- `cargo run -q -- --run examples/run_soma.pink`
- `cargo run -q -- --run examples/run_chamada.pink`

## Resultado real
- Todos os comandos passaram.
- `run_global.pink` imprime `100`.
- `run_global_expr.pink` imprime `44`.
- Regressão de `run_soma.pink` e `run_chamada.pink` mantida.

## Próximos passos sugeridos
- Teste dedicado para erro de valor global não suportado, caso surja formato adicional no lowering.
- Se necessário no futuro, estudar escrita em globals com semântica explícita (fora do escopo atual).
