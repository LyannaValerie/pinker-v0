# Handoff Auditor

## Rodada
- fase: Fase 16 — robustez do interpretador e testes negativos de runtime
- escopo auditado: src/interpreter.rs, tests/interpreter_tests.rs, pipeline completa `--run`
- estado auditado: Fase 15 concluída, 149 testes passando, build limpo

## Como me situei
- Lidos: README.md, docs/agent_state.md, docs/handoff_codex.md, docs/phases.md, src/main.rs, src/interpreter.rs, src/error.rs, tests/interpreter_tests.rs
- Executados: `cargo build` (limpo) e `cargo test` (149 passou, 0 falhou)
- Fonte de verdade: código real. Sem divergências relevantes encontradas.

## Confirmado no código
- `interpreter.rs` tem 250 linhas e cobre todas as 19 instruções da Machine
- Todos os terminadores implementados: `Jmp`, `BrTrue`, `Ret`, `RetVoid`
- 9 guards de erro existem no código com mensagens definidas, mas sem teste cobrindo-os
- `runtime_err` fixa span em `1:1` para todos os erros de runtime — sem contexto de função/bloco
- `fn_name` está disponível em `call_function` mas não aparece em nenhuma mensagem de erro
- 17 testes existentes em `interpreter_tests.rs`: 13 positivos via `run_code`, 2 negativos via Machine manual, 2 via CLI

## Lacunas principais
- **9 caminhos de erro sem teste**: divisão por zero, slot não inicializado, aridade inválida, `call` para void, `call_void` para não-void, `ret` com pilha vazia ou >1 valor, `ret_void` com pilha suja, label inexistente, valor de global não suportado
- **7 instruções sem cobertura end-to-end**: `Not`, `Div`, `CmpEq`, `CmpNe`, `CmpLe`, `CmpGt`, `CmpGe`
- **Sem cobertura de variável mutável** (`mut` com reassignment via `StoreSlot` em slot já existente)
- **Contexto de erro pobre**: erros de runtime não identificam função, bloco ou instrução causadora

## Testes negativos indispensáveis
Todos via construção manual de `MachineProgram` (mesmo padrão de `run_falha_funcao_inexistente`):

1. `run_falha_divisao_por_zero` — `Div` com 0 no rhs → mensagem contém `"divisão por zero"`
2. `run_falha_slot_nao_inicializado` — `LoadSlot` em slot sem store anterior → `"load_slot em slot não inicializado"`
3. `run_falha_call_retorna_void` — `Call` para função que retorna `None` → `"call exige função com retorno"`
4. `run_falha_call_void_retorna_valor` — `CallVoid` para função que retorna `Some(...)` → `"call_void exige função sem retorno"`
5. `run_falha_aridade_invalida` — `call_function` com argc diferente de params.len() → `"chamada com aridade inválida"`
6. `run_falha_valor_global_nao_suportado` — `MachineGlobal` com `OperandIR` que não é `Int` nem `Bool` → `"valor global não suportado em runtime"`

## Testes end-to-end indispensáveis
Via `run_code(source)` (pipeline completa do código-fonte):

1. `run_not_unario` — `!verdade` → `false` como condição; cobre instrução `Not` sem cobertura atual
2. `run_divisao` — `10 / 2` → `5`; cobre instrução `Div` sem cobertura atual
3. `run_igualdade` — `1 == 1` em `talvez`; cobre `CmpEq`
4. `run_diferenca` — `1 != 2` em `talvez`; cobre `CmpNe`
5. `run_comparacoes_restantes` — pelo menos um caso com `>=`, `>`, `<=`; cobre `CmpGe`, `CmpGt`, `CmpLe`
6. `run_variavel_mutavel` — `mut x = 1; x = 99; mimo x;` → `99`; cobre reassignment

Opcional de alto valor: `cli_run_erro_runtime_tem_exit_nonzero` — verifica que `--run` com programa que falha em runtime retorna exit code não-zero e escreve em stderr.

## Pequenos endurecimentos aceitáveis
- **Incluir nome da função nas mensagens de erro** em `call_function`: `fn_name` já está no escopo em todos os sites de erro. Trocar `runtime_err("msg")` por `runtime_err(&format!("[{}] msg", fn_name))`. Mudança de string apenas, sem alteração de assinatura ou tipo.
- Nenhum outro endurecimento estrutural é necessário nesta fase.

## O que NÃO deve entrar nesta fase
- Escrita em globals (`store_global`) — abre semântica de mutação
- I/O de linguagem — requer extensão de sintaxe
- Suporte a `eterno` (loops) no interpretador — requer lowering novo
- Proteção contra recursão infinita / profundidade máxima de call — requer decisão de design
- Debugger, stepping, tracing
- Inferência de tipo cross-block na Machine
- Mudanças estruturais em `error.rs` (novos campos no `Runtime` variant)
- Qualquer mudança em frontend, parser, semântica ou camadas de lowering
- Backend nativo, LLVM, Cranelift

## Divergências entre docs e código real
- Nenhuma divergência relevante. `docs/handoff_codex.md` e `docs/agent_state.md` refletem corretamente o estado do código após Fase 15.
- Nota: `docs/handoff_opus.md` não foi lido por não estar na lista obrigatória, mas está referenciado em `agent_state.md` como leitura para o Codex. Não afeta esta auditoria.

## Recomendação ao Codex
Mexer em:
- `tests/interpreter_tests.rs`: adicionar 6 testes negativos via Machine manual (div zero, slot não init, aridade, call/call_void mismatch, global inválida)
- `tests/interpreter_tests.rs`: adicionar 6 testes end-to-end via `run_code` (Not, Div, CmpEq, CmpNe, comparações restantes, mut reassignment)
- `src/interpreter.rs`: opcional — incluir `fn_name` nas mensagens de erro em `call_function` (mudança de string, sem alteração de API)

Não mexer em:
- `error.rs`, `main.rs`, `abstract_machine.rs`, `semantic.rs` ou qualquer camada de frontend/lowering

Menor diff aceitável:
- `tests/interpreter_tests.rs` com ~12 novos testes, todos passando
- `src/interpreter.rs` intocado ou com mudança mínima de string
- `cargo test` verde ao final
- Atualizar `docs/handoff_codex.md`, `docs/agent_state.md` e `docs/phases.md` ao concluir

## Comandos executados
- cargo build: `Finished dev profile — 0 erros, 0 warnings`
- cargo test: `149 passed; 0 failed` (17 em interpreter_tests, 132 nos demais módulos)
