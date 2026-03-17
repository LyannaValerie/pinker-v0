# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 16**: robustez do interpretador e cobertura negativa de runtime.

## Objetivo
- Aumentar confiança no runtime sem redesign:
  - ampliar testes negativos;
  - ampliar cenários end-to-end com `--run`;
  - endurecer pontos permissivos mínimos quando necessário.

## Nota de auditoria
- `docs/handoff_auditor.md` **não existe** no estado atual do repositório.
- Fallback adotado: usar código real + handoff vigente como fonte de verdade.

## Arquivos alterados
- `src/interpreter.rs`
- `tests/interpreter_tests.rs`
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Endurecimentos aplicados
- Runtime agora rejeita globals duplicadas ao montar mapa de globals (`global duplicada em runtime`).

## Casos negativos agora cobertos
- divisão por zero em runtime;
- função inexistente;
- política de `call_void` (erro ao chamar função com retorno);
- aridade inválida em chamada;
- global inexistente;
- valor global não suportado em runtime;
- global duplicada em runtime.

## End-to-end (`--run`) ampliado
- caso já existente de chamada entre funções;
- caso já existente de global simples;
- novo teste CLI para global em expressão (`44`);
- novo teste CLI para chamada void como statement (`42`).

## O que o interpretador cobre agora
- execução de `principal` via `--run`;
- controle de fluxo básico, pilha e slots;
- chamadas entre funções (`call`/`call_void`) com política explícita;
- leitura de globals literais (`int`/`bool`) em runtime.

## Limites que permanecem
- sem escrita em globals;
- sem I/O na linguagem;
- sem infraestrutura avançada de runtime (debug/step/tracing).

## Testes/comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`
- `cargo run -q -- --run examples/run_soma.pink`
- `cargo run -q -- --run examples/run_chamada.pink`
- `cargo run -q -- --run examples/run_global.pink`
- `cargo run -q -- --run examples/run_global_expr.pink`

## Resultado real
- Todos os comandos passaram.

## Próximos passos sugeridos
- Se o lowering de globals evoluir, adicionar suporte explícito aos novos formatos em `eval_global_value` com testes dedicados.
- Considerar testes negativos de CLI para erros de runtime (assert de stderr) em rodada futura.
