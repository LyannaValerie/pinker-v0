# Handoff Auditor

## Rodada
- fase: Fase 17 — recursão com teste dedicado e robustez de chamadas no interpretador
- escopo auditado: src/interpreter.rs, tests/interpreter_tests.rs, pipeline completa `--run`
- estado auditado: Fase 16 concluída, 164 testes passando, build limpo

## Como me situei
- Lidos: README.md, docs/agent_state.md, docs/handoff_codex.md, docs/phases.md, src/main.rs, src/interpreter.rs, src/error.rs, tests/interpreter_tests.rs
- Executados: `cargo build` (limpo) e `cargo test` (164 passou, 0 falhou)
- Testado manualmente: fat(5) → 120, fib(7) → 13, recursão mútua eh_par(4) → 1
- Fonte de verdade: código real do workspace. Sem divergências relevantes encontradas.

## Confirmado no código
- Recursão já funciona sem redesign: `call_function` é chamada recursivamente via `exec_instr::Call`, e cada chamada cria novos `slots`/`stack`/`labels` isolados na Rust call stack
- Recursão mútua também funciona: funções se enxergam pelo `program.functions` compartilhado
- Sem nenhum teste dedicado de recursão em `interpreter_tests.rs` (32 testes, nenhum cobre auto-chamada)
- Sem nenhum exemplo `.pink` de função recursiva em `examples/`
- Recursão infinita causa `stack overflow` do Rust (panic fatal, não `PinkerError`) — comportamento esperado para um interpretador sem limite de profundidade
- `find_function` faz busca linear O(n) por nome — irrelevante para programas pequenos, sem nenhum risco para esta fase
- `CmpLt` já coberto por `run_comparacao_em_fluxo_de_controle` (Fase 13), sem lacuna

## Lacunas principais
- **Zero testes de recursão**: fatorial, fibonacci, recursão com acumulador — nenhum coberto
- **Zero testes de recursão mútua**: duas funções que se chamam entre si — nenhum coberto
- **Sem exemplo `.pink` de recursão**: `examples/run_fatorial.pink` e afins não existem
- **Recursão infinita gera panic do Rust**, não `PinkerError` — limite documentado mas sem teste que verifica o comportamento atual (e.g., que o processo termina com código não-zero)

## Testes de recursão indispensáveis
Todos via `run_code(source)`:

1. `run_recursao_fatorial` — `fat(5)` → `120`; cobre auto-chamada com base `n == 0`
2. `run_recursao_fibonacci` — `fib(7)` → `13`; cobre recursão dupla com dois casos base
3. `run_recursao_com_acumulador` — `soma(n)` retorna `n + (n-1) + ... + 1`; cobre recursão linear simples sem ramos duplos

Opcional de alto valor:
4. `run_recursao_mutua` — `eh_par(4)` via `eh_par`/`eh_impar` mutuamente recursivas → `1`; confirma que o programa compartilhado é acessível recursivamente

## Testes end-to-end indispensáveis
Via `cargo run -q -- --run`:

1. `examples/run_fatorial.pink` — `fat(5)` → imprime `120`
2. `examples/run_fibonacci.pink` — `fib(7)` → imprime `13`

Esses arquivos devem ser criados em `examples/` e executados nos comandos obrigatórios ao final.

## Pequenos endurecimentos aceitáveis
- Nenhum endurecimento estrutural é necessário nesta fase: o interpretador já suporta recursão corretamente
- Opcional mínimo: adicionar comentário inline em `call_function` explicando que a recursão usa a Rust call stack e que não há proteção contra stack overflow — apenas documentação, sem mudança de código

## O que NÃO deve entrar nesta fase
- Limite de profundidade de chamada (`max_depth`) — requer decisão de design (qual limite? qual erro?)
- Proteção contra recursão infinita com `RuntimeError` — fora do escopo atual
- Otimização de tail call — fora do escopo
- Loops (`eterno`) no interpretador — requer lowering novo
- Escrita em globals
- I/O de linguagem
- Debugger, tracing
- Qualquer mudança em frontend, parser, semântica ou camadas de lowering
- Backend nativo, LLVM, Cranelift

## Divergências entre docs e código real
- Nenhuma divergência. `docs/handoff_codex.md` e `docs/agent_state.md` refletem corretamente o estado pós-Fase 16.
- `docs/handoff_codex.md` menciona como próximos passos "loops (eterno)" e "overflow wrapping". Ambos continuam adiados. Esta fase prioriza recursão.

## Recomendação ao executor
Mexer em:
- `tests/interpreter_tests.rs`: adicionar 3 testes via `run_code` (fatorial, fibonacci, recursão linear) + 1 opcional (recursão mútua)
- `examples/run_fatorial.pink`: criar arquivo simples com `fat(5)` → `120`
- `examples/run_fibonacci.pink`: criar arquivo simples com `fib(7)` → `13`

Não mexer em:
- `src/interpreter.rs` — nenhuma mudança necessária; recursão já funciona
- `error.rs`, `main.rs`, camadas de lowering, frontend

Menor diff aceitável:
- `tests/interpreter_tests.rs` com 3–4 novos testes, todos passando
- 2 novos arquivos `.pink` em `examples/`
- `src/interpreter.rs` intocado
- `cargo test` verde ao final
- Atualizar `docs/handoff_codex.md`, `docs/agent_state.md` e `docs/phases.md` ao concluir

## Comandos executados
- cargo build: `Finished dev profile — 0 erros, 0 warnings`
- cargo test: `164 passed; 0 failed` (32 em interpreter_tests, 132 nos demais)
