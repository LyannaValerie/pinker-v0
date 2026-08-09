# Mapa curto de código

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Referência rápida para localizar a camada certa antes de editar.

## Frontend

- tokens e spans: `src/token.rs`
- léxico: `src/lexer.rs`
- AST: `src/ast.rs`
- parser: `src/parser.rs`

## Semântica e layout

- checagem semântica principal: `src/semantic.rs`
- cargas de variantes de `leque`: autoridade única de resolução/classificação em `src/enum_payload.rs`, identidade e metadata em `src/ir.rs`, invariantes em `src/ir_validate.rs`, `src/cfg_ir_validate.rs`, `src/instr_select_validate.rs` e `src/abstract_machine_validate.rs`
- layout de tipos compostos: `src/layout.rs`
- erros/renderização comum: `src/error.rs`, `src/printer.rs`

## Pipeline intermediário

- IR estruturada: `src/ir.rs`, `src/ir_validate.rs`
- CFG IR: `src/cfg_ir.rs`, `src/cfg_ir_validate.rs`
- seleção de instruções: `src/instr_select.rs`, `src/instr_select_validate.rs`
- máquina abstrata: `src/abstract_machine.rs`, `src/abstract_machine_validate.rs`
- ponteiros crus de função (Fase 245): metadados e lowering atravessam todas as camadas acima, com `CallRaw`

## Execução e saída

- interpretador `--run`: `src/interpreter.rs`
- subprocessos mínimos e argv explícito conservador em 16.2 (`executar_processo`, `executar_com_entrada`, `capturar_stdout`, `capturar_stderr`): `src/interpreter.rs`
- REPL mínimo `pink repl`: `src/repl.rs`
- backend textual final: `src/backend_text.rs`, `src/backend_text_validate.rs`
- backend `.s`: `src/backend_s.rs`
- ponteiros crus de função (Fase 245): formação em `src/parser.rs`, contrato em `src/semantic.rs`, lowering em `src/ir.rs`, execução determinística em `src/interpreter.rs` e ABI indireta em `src/backend_s.rs`
- memória pública (Fase 246 + hotfix extraordinário): semântica/lowering em `src/semantic.rs` e `src/ir.rs`; contrato puro de páginas, identidades, virtual vitalício, bytes vivos e metadata em `runtime/pinker_memory_contract/src/lib.rs`; regiões esparsas e contabilidade equivalente em `src/interpreter.rs`; mapeamentos anônimos proporcionais, lazy, sem reuso, com validação de vida/limites/alinhamento em `runtime/pinker_rt/src/lib.rs`
- contenção do host nas suítes nativas: autoridade e outcome em
  `tests/common/native_process.rs`, causa/eventos tipados em
  `tests/common/native_process_model.rs`, launcher/PGID em
  `tests/common/native_process_launcher.rs` e evidência repetível em
  `scripts/pinker-flake-runner.sh`;
  recuperação conservadora em `scripts/pinker-cleanup.sh`; core zero da
  esteira em `ci_env.sh`
- assembly inline (Fase 247): `sussurro` atravessa AST, IR, CFG, seleção e máquina; validação em `src/semantic.rs`, erro hospedado em `src/interpreter.rs` e emissão GNU Intel x86-64 em `src/backend_s.rs`
- uniões estruturais (Fase 248): contrato normativo único de canonicalização em `src/union_canon.rs`, consumido pela semântica (`src/semantic.rs`) e pelo lowering (`src/ir.rs`); registry internado em `src/ir.rs`, preservação/validação nas camadas intermediárias, descritores hospedados em `src/interpreter.rs` e ABI interna em `runtime/pinker_rt/src/lib.rs`
- `encaixe` de união tipado (HR1 da revisão humana da PR #411): nó próprio na AST (`src/ast.rs`), preservação no parser (`src/parser.rs`), resolução de apelidos e cobertura canônica em `src/semantic.rs`, associação ao registry e operações internas tipadas (`UnionMatch`/`UnionTag`/`UnionExtract`) em `src/ir.rs`, propagação por `src/cfg_ir.rs`, `src/instr_select.rs` e `src/abstract_machine.rs`, execução direta em `src/interpreter.rs` e escolha do símbolo de ABI apenas em `src/backend_s.rs`
- identidade semântica de tipos (HR4 da revisão humana da PR #411): chave canônica exaustiva e transparência de apelidos em `src/union_canon.rs`; `ResolvedTypeId`, `ResolvedTypeIR`, `TypeRefIR`, `ResolvedTypeTable`, a tabela `ProgramIR.resolved_types` e a seleção do membro por igualdade exata em `src/ir.rs`; transporte e verificação da identidade do membro por `src/cfg_ir.rs`, `src/instr_select.rs`, `src/abstract_machine.rs`, `src/ir_validate.rs`, `src/instr_select_validate.rs` e `src/interpreter.rs`. `TypeIR` permanece apenas a categoria operacional; a ABI de payload não muda
- payloads estruturais de união (HR3 da revisão humana da PR #411): classificação única e exaustiva das representações (escalar, handle opaco, agregado), layout real, transparência de apelidos em profundidade e limites documentados em `src/union_payload.rs`; rejeição antecipada com códigos `E-SEMANTIC-UNION-PAYLOAD-*` em `src/semantic.rs`; `UnionPayloadLayout` transportado por `src/ir.rs`, `src/cfg_ir.rs`, `src/instr_select.rs` e `src/abstract_machine.rs` e reconferido pelos validadores correspondentes; snapshot independente e orçamento no interpretador (`src/interpreter.rs`); scratch alinhado de injeção, storage novo de extração e ABI por endereço em `src/backend_s.rs`; descritor de bloco único com registro de handles, validação e orçamentos em `runtime/pinker_rt/src/lib.rs`
- atribuição de símbolo em `sussurro` (hotfix pós-PR #412): política estrutural das três formas de statement do GNU as e invariante do artefato em `src/inline_asm.rs`, leitor próprio de ELF64 sem dependência externa em `src/elf.rs` e cabo da verificação no caminho real de `pink build --nativo` em `src/main.rs`
- boot/freestanding: `src/boot.rs`
- CLI: `src/main.rs`

## Editor/TUI

- editor oficial mínimo: `src/editor_tui.rs`
- paleta/tema: `src/palette.rs`

## Testes por camada

- frontend: `tests/lexer_tests.rs`, `tests/parser_tests.rs`
- semântica: `tests/semantic_tests.rs`
- IR/CFG/seleção: `tests/ir_tests.rs`, `tests/cfg_ir_tests.rs`, `tests/instr_select_tests.rs`
- máquina/runtime: `tests/abstract_machine_tests.rs`, `tests/abstract_machine_stack_tests.rs`, `tests/interpreter_tests.rs`
- backends: `tests/backend_text_tests.rs`, `tests/backend_s_tests.rs`, `tests/backend_s_external_toolchain_tests.rs`
- Fases 245–246: `tests/phase245_246_tests.rs`
- Fases 247–248 e correções da revisão humana da PR #411: `tests/phase247_248_tests.rs`
- hotfix de memória pública: `tests/public_memory_hotfix_tests.rs` e testes de unidade em `runtime/pinker_memory_contract`/`runtime/pinker_rt`
- contenção do host: autoridade, outcome tipado, lifecycle e allowlists de FDs
  em `tests/common/native_process.rs`, `tests/common/native_process_model.rs` e
  `tests/common/native_process_launcher.rs`; regressões em
  `tests/native_process_control_tests.rs`, `tests/native_cleanup_tests.rs` e
  `tests/core_dump_policy_tests.rs`; runner finito e evidência automática em
  `scripts/pinker-flake-runner.sh`
- expansão D1 de cargas `lista<E>` em leques: `tests/d1_leque_carga_lista_tests.rs` (matrizes positiva/negativa, IR, validadores, ABI e paridade)
- índice derivado de símbolos: metadata e catálogo em `src/nav.rs`, modelo,
  derivação e renderização em `src/symbol_index.rs`, adaptador CLI em
  `src/main.rs` e contrato processual em `tests/symbol_index_cli_tests.rs`
- cobertura de diff somente leitura: parser, modelo, derivação e renderização
  em `src/diff_coverage.rs`, adaptador stdin/CLI em `src/main.rs` e contrato
  processual em `tests/diff_coverage_cli_tests.rs`
- CLI/saída: `tests/output_tests.rs`, `tests/editor_tui_tests.rs`

## Docs que costumam acompanhar mudança funcional

- estado e regras: `docs/doc_rules.md`, `docs/handoff_codex.md`
- ordem e continuidade: `docs/roadmap.md`, `docs/history.md`
- uso e navegação: `README.md`, `MANUAL.md`, `docs/atlas.md`
