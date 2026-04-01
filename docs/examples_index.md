# Índice rápido de exemplos e testes

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Guia curto para localizar cobertura útil sem varrer `examples/` e `tests/` inteiros.

| Área | Exemplo(s) | Teste(s) | Cobertura |
|---|---|---|---|
| parser / AST | `examples/principal_valida.pink`, `examples/principal_invalida.pink` | `tests/parser_tests.rs` | parse básico, erros de sintaxe |
| semântica base | `examples/mut_ok.pink`, `examples/mut_falho.pink` | `tests/semantic_tests.rs` | `principal`, tipos, mutabilidade, aridade |
| módulos / imports | `examples/fase60_modulos_valido.pink`, `examples/fase60_modulo_ausente.pink` | `tests/interpreter_tests.rs`, `tests/semantic_tests.rs` | `trazer`, símbolo ausente, integração CLI |
| `verso` / texto | `examples/fase88_verso_operacional_minimo_valido.pink`, `examples/fase137_split_camada1_valido.pink`, `examples/fase140_busca_textual_camada1_valido.pink` | `tests/interpreter_tests.rs`, `tests/semantic_tests.rs` | texto no runtime e intrínsecas textuais |
| runtime / `--run` | `examples/run_soma.pink`, `examples/run_sempre_que.pink`, `examples/run_global.pink` | `tests/interpreter_tests.rs` | execução ponta a ponta |
| processos externos mínimos | `examples/fase161_processo_externo_minimo_valido.pink`, `examples/fase161_processo_externo_fluxo_composto_valido.pink`, `examples/fase163_captura_stdout_minima_valido.pink`, `examples/fase163_captura_stdout_fluxo_composto_valido.pink`, `examples/fase164_captura_stderr_minima_valido.pink`, `examples/fase164_captura_stderr_fluxo_composto_valido.pink`, `examples/fase165_stdin_textual_minimo_valido.pink`, `examples/fase165_stdin_textual_fluxo_composto_valido.pink` | `tests/interpreter_tests.rs`, `tests/semantic_tests.rs` | `executar_processo(comando)`, `executar_com_entrada(comando, entrada)`, `capturar_stdout(comando)`, `capturar_stderr(comando)`, retorno de código, captura textual mínima de stdout/stderr, envio mínimo de stdin textual, fluxo composto e portabilidade com binários auxiliares do repositório |
| memória explícita | `examples/fase66_deref_leitura_valido.pink`, `examples/fase67_escrita_indireta_valida.pink`, `examples/fase71_cast_memoria_valido.pink` | `tests/interpreter_tests.rs`, `tests/cfg_ir_tests.rs` | ponteiro, deref, cast útil |
| backend textual | `examples/emit_if_else.pink`, `examples/selected_if_else.pink`, `examples/machine_if_else.pink` | `tests/backend_text_tests.rs`, `tests/abstract_machine_tests.rs` | `--selected`, `--machine`, `--pseudo-asm` |
| backend `.s` externo | `examples/fase112_branch_condicional_minimo_valido.pink`, `examples/fase135_verso_camada1_valido.pink` | `tests/backend_s_tests.rs`, `tests/backend_s_external_toolchain_tests.rs` | `--asm-s`, ABI e subset montável |
| coleções e dados | `examples/fase149_lista_minima_bombom_valido.pink`, `examples/fase152_mapa_verso_bombom_minimo_valido.pink`, `examples/fase160_tempo_basico_timestamp_valido.pink` | `tests/interpreter_tests.rs`, `tests/semantic_tests.rs` | listas, mapa, formatação, JSON/CSV, tempo |

Regra prática:
- mudança em parser/léxico: começar por `tests/parser_tests.rs`
- mudança em semântica/intrínseca: começar por `tests/semantic_tests.rs` e `tests/interpreter_tests.rs`
- mudança em lowering/backend: começar pelo exemplo versionado da fase e pelo teste da camada correspondente
