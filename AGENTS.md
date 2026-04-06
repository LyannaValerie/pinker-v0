# AGENTS.md

Guia operacional curto para agentes neste repositório. Não substitui `README.md`, `MANUAL.md` ou os docs canônicos de `docs/`.

## Comandos padrão

```bash
make preflight
make build
make test
make fmt-check
make clippy
make ci
make run-example EX=examples/principal_valida.pink
make check-example EX=examples/principal_valida.pink
make audit-example EX=examples/principal_valida.pink
make smoke
```

Sem `make`:

```bash
./ci_env.sh --preflight
./ci_env.sh cargo build --locked
./ci_env.sh cargo test --locked
./ci_env.sh cargo fmt --check
./ci_env.sh cargo clippy --all-targets --all-features -- -D warnings
./ci_env.sh cargo run --bin pink -- examples/principal_valida.pink
./ci_env.sh cargo run --bin pink -- --check examples/principal_valida.pink
```

## Contrato operacional da suíte

- A suíte oficial é **stable-only** no toolchain fixado pelo repositório.
- Não depender de nightly nem de `-Z unstable-options`.
- O caminho oficial precisa passar por `./ci_env.sh`, que saneia `RUSTFLAGS` e `CARGO_ENCODED_RUSTFLAGS` e expõe preflight mínimo de diagnóstico.

## Mapa rápido do código

- parser/léxico/AST: `src/token.rs`, `src/lexer.rs`, `src/ast.rs`, `src/parser.rs`
- semântica/layout: `src/semantic.rs`, `src/layout.rs`
- IR/CFG/seleção/máquina: `src/ir.rs`, `src/cfg_ir.rs`, `src/instr_select.rs`, `src/abstract_machine.rs`
- validações de pipeline: `src/ir_validate.rs`, `src/cfg_ir_validate.rs`, `src/instr_select_validate.rs`, `src/abstract_machine_validate.rs`
- backends/runtime/CLI: `src/backend_text.rs`, `src/backend_s.rs`, `src/interpreter.rs`, `src/main.rs`
- testes: `tests/parser_tests.rs`, `tests/semantic_tests.rs`, `tests/interpreter_tests.rs`, `tests/backend_s_external_toolchain_tests.rs`

Mapa mais curto por feature: `docs/code_map.md`.
Índice rápido de exemplos/testes: `docs/examples_index.md`.

## Regras locais de mudança

- Preservar continuidade factual do workspace e da trilha ativa.
- Tarefa operacional não abre fase, Doc, FE ou HF.
- Não tocar docs canônicos por inércia: `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/roadmap.md`, `docs/future.md`, `docs/phases.md`.
- Mudança funcional real exige evidência em código, testes e docs canônicos apropriados.
- Não reverter mudanças do usuário sem pedido explícito.
- Validar com `build`, `test`, `fmt-check` e `clippy` antes de encerrar.

## O que sempre checar em mudança funcional

- `README.md`
- `MANUAL.md`
- `docs/doc_rules.md`
- `docs/atlas.md`
- `docs/roadmap.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`
- `docs/history.md`
- exemplos e testes afetados

## O que normalmente não tocar em tarefa operacional

- `docs/history.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`
- `docs/roadmap.md`
- `docs/future.md`
- `docs/phases.md`

## Fluxo curto recomendado

1. Ler `README.md`, `docs/atlas.md`, `docs/agent_state.md`, `docs/handoff_codex.md` e `docs/doc_rules.md`.
2. Rodar `make ci`.
3. Localizar a camada afetada em `docs/code_map.md`.
4. Escolher um exemplo/teste próximo em `docs/examples_index.md`.
5. Fazer o menor diff auditável.
6. Revalidar e só atualizar docs canônicos se a tarefa for funcional/documental de verdade.

## Checklist de fechamento

- código alterado no menor recorte útil
- testes/exemplos ajustados, se aplicável
- docs canônicos atualizados apenas se aplicável
- `make ci` executado
- diff auditável
- continuidade preservada
