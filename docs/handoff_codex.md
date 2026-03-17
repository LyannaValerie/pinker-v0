# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 14**: suporte a chamadas entre funções no interpretador da Machine.

## Objetivo
- Permitir `MachineInstr::Call` e `MachineInstr::CallVoid` com frame por função, mantendo arquitetura simples.
- Preservar pipeline atual (`--run` reaproveita semântica → IR → CFG → seleção → Machine → validação Machine).

## Arquivos alterados
- `src/interpreter.rs`
- `tests/interpreter_tests.rs`
- `examples/run_chamada.pink`
- `examples/run_chamada_void.pink`
- `README.md`
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Diagnóstico real encontrado
- Fase 13 estava estável e já executava `principal`, mas bloqueava `call`/`call_void` por erro explícito.
- O lowering já emite argumentos em ordem de origem e a pilha recebe `arg1, arg2, ...`; portanto a rotina de chamada no runtime precisa desempilhar e reverter para reconstruir a ordem lógica.

## Decisão técnica aplicada
- Extraída rotina reutilizável `call_function(fn_name, args, program)` no interpretador.
- `run_program` agora valida globals e despacha para `call_function("principal", vec![], program)`.
- `exec_instr` recebeu acesso ao `MachineProgram` para resolver chamadas.

## Cobertura atual do interpretador
- Continua cobrindo instruções básicas da Fase 13.
- Agora cobre também:
  - `Call`: consome `argc`, reordena argumentos, chama função alvo, exige retorno com valor e empilha no chamador.
  - `CallVoid`: consome `argc`, reordena argumentos e chama função alvo.

## Política adotada para `CallVoid`
- `call_void` **exige função sem retorno** no runtime.
- Se a função chamada retornar valor (`Some`), o interpretador falha com erro curto: `call_void exige função sem retorno`.
- Esta política é consistente com validador/lowering atuais (que já tratam `call_void` como chamada para função `nulo`).

## O que ainda não cobre
- globals (erro explícito).
- I/O na linguagem.
- infraestrutura avançada de runtime (debugger, otimizações, etc.).

## Limites remanescentes
- Sem suporte a globals em execução.
- Sem política avançada de overflow além do comportamento atual com `wrapping_*` nas operações inteiras.

## Testes/comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`
- `cargo run -q -- --run examples/run_chamada.pink`
- `cargo run -q -- --run examples/run_soma.pink`
- `cargo run -q -- --run examples/run_chamada_void.pink`

## Resultado real
- Todos os comandos passaram.
- `run_chamada.pink` e `run_soma.pink` imprimem `42`.
- `run_chamada_void.pink` imprime `42`.

## Próximos passos sugeridos
- Adicionar testes dedicados para recursão (não foco desta fase, mas a estrutura de chamadas já permite execução empilhada).
- Definir política explícita de impressão para possíveis retornos `lógica` em `principal` caso a semântica futura permita.
