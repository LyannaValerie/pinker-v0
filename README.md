# Pinker v0

Pinker v0 é um frontend pequeno e congelado em Rust para a linguagem Pinker.

## O que o frontend faz hoje
- léxico com spans
- parser para `pacote`, `trazer`, `carinho`, `mimo`, `talvez/senão`, `sempre que`, `eterno`, `nova`, `mut`
- tipos `bombom`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` e `logica`
- aliases de tipo (`apelido`), arrays fixos (`[tipo; N]`), structs (`ninho`), ponteiros (`seta<tipo>`)
- representação mínima de ponteiro no runtime (`RuntimeValue::Ptr`) para `seta<T>` no `--run`
- dereferência de leitura mínima com `*p` para `seta<bombom>` no `--run`
- escrita indireta mínima com `*p = valor` para `seta<bombom>` no `--run`
- aritmética mínima de ponteiro no runtime com `seta<bombom> + bombom` e `seta<bombom> - bombom` no `--run`
- acesso operacional mínimo a campo de `ninho` no runtime via `(*ptr).campo`, respeitando offsets de layout estático no subset da fase
- indexação operacional mínima de arrays no runtime via `(*ptr)[i]`, reaproveitando aritmética de ponteiros + `deref_load` no subset `[bombom; N]` com índice `bombom`
- qualificador `fragil` (`volatile`) para ponteiros explícitos (`fragil seta<tipo>`), com efeito operacional mínimo em `deref_load`/`deref_store` via caminho distinto no pipeline/runtime para o subset `fragil seta<bombom>`
- inline asm mínimo como statement textual com `sussurro("...")` (ou múltiplas strings), preservado até IR
- marca de unidade freestanding/no-std com `livre;` no topo do programa
- cast explícito controlado com `virar` (operacional em `--run` para inteiro->inteiro e `bombom <-> seta<bombom>`)
- consultas estáticas de layout com `peso(tipo)` e `alinhamento(tipo)`
- módulos/imports mínimos com `trazer modulo;` e `trazer modulo.simbolo;` (carregando `modulo.pink` no mesmo diretório do arquivo principal, com subset de import para `carinho` e `eterno`)
- strings mínimas como valor de linguagem com tipo `verso` e literal `"texto"` (frontend + semântica + IR)
- saída básica com `falar(expr);` para `bombom`, `u8`, `u16`, `u32`, `u64`, `logica` e `verso` (executa em `--run`)
- comando de projeto `pink build <arquivo.pink>` para gerar artefato textual `.s` em disco (padrão: `build/<arquivo>.s`)
- chamadas diretas por nome
- checagem semântica de `principal`, retorno, mutabilidade, aridade e tipos
- AST textual estável
- AST JSON estável
- IR estruturada + validação interna
- CFG IR + validação interna
- seleção de instruções textual + validação
- alvo textual abstrato (máquina de pilha) + validação estrutural e disciplina de pilha
- backend textual pseudo-assembly + validacao interna
- proteção preventiva de recursão no runtime (`--run`) com limite interno de profundidade de chamadas
- metadata mínima de boot entry + linker script textual em modo `livre` na saída `--asm-s` (Fase 58)

## O que não faz
- codegen nativo real
- backend nativo
- LLVM / Cranelift
- otimizações grandes
- FFI, enums, generics, traits
- operações completas de ponteiro (aritmética além do subset mínimo da fase, como `n + ptr`, `ptr - ptr`), acesso completo via ponteiro (`seta<T>`), escrita em campo/index, layout físico/ABI
- acesso operacional de campo de `ninho` além do subset da fase (ex.: base por valor `p.campo`, escrita de campo, campos não escalares)
- indexação operacional de arrays além do subset da fase (ex.: base por valor `arr[i]`, escrita por índice e elementos não `bombom`)
- leitura indireta além do subset mínimo da fase (`*p` apenas para `seta<bombom>` com endereçamento abstrato de globals escalares no runtime)
- escrita indireta além do subset mínimo da fase (`*p = v` apenas para `seta<bombom>` com endereçamento abstrato de globals escalares já mapeadas no runtime)
- semântica completa de `fragil` em runtime/backend (nesta fase há apenas efeito operacional mínimo em acessos indiretos no subset `fragil seta<bombom>`, sem MMIO/fences/ordenação de memória)
- lowering operacional de `virar` fora do subset atual (nesta fase executa inteiro->inteiro e `bombom <-> seta<bombom>`; demais casts continuam rejeitados)
- lowering operacional de inline asm em CFG/Machine/runtime (`--check`/`--ir` aceitam o subset da fase; `--cfg-ir`/`--run` ainda não executam `sussurro`)
- lowering operacional de `verso` em CFG/Machine/runtime além de `falar`: `verso` como valor geral (passagem por chamada, retorno, variável) ainda não executa em `--cfg-ir`/`--run`; apenas `falar("literal")` funciona em `--run`
- I/O de leitura (`ouvir`), arquivo (`abrir`, `fechar`, `escrever`) e formatação avançada de saída
- freestanding/no-std operacional real (nesta fase `livre;` é marca semântica de intenção, não runtime bare-metal executável)

## Build e testes
```bash
cargo build
cargo test
```

## CI + MSRV
- CI em `.github/workflows/ci.yml` rodando: `cargo build --locked`, `cargo check --locked`, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --locked` e `cargo doc --no-deps -D warnings`.
- MSRV adotada: **Rust 1.78.0** (fixada em `rust-toolchain.toml`).

### Comandos locais equivalentes ao CI
```bash
cargo build --locked
cargo check --locked
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

## Uso
```bash
cargo run -- examples/principal_valida.pink
cargo run -- --ir examples/ir_if_else.pink
cargo run -- --cfg-ir examples/cfg_if_else.pink
cargo run -- --selected examples/selected_if_else.pink
cargo run -- --machine examples/machine_if_else.pink
cargo run -- --machine examples/machine_stack_if_call.pink
cargo run -- --pseudo-asm examples/emit_if_else.pink
cargo run -- --asm-s examples/emit_if_else.pink
cargo run -- --run examples/run_soma.pink
cargo run -- --run examples/run_chamada.pink
cargo run -- --run examples/run_sempre_que.pink
cargo run -- --run examples/run_quebrar.pink
cargo run -- --run examples/run_continuar.pink
cargo run -- --run examples/run_global.pink
cargo run -- --run examples/run_unsigned_basico.pink
cargo run -- --run examples/run_signed_basico.pink
cargo run -- --run examples/run_alias_tipo_basico.pink
cargo run -- --run examples/fase64_falar_signed.pink
cargo run -- --run examples/fase66_deref_leitura_valido.pink
cargo run -- --run examples/fase67_escrita_indireta_valida.pink
cargo run -- --run examples/fase68_ptr_aritmetica_valida.pink
cargo run -- --run examples/fase68_ptr_aritmetica_leitura_valida.pink
cargo run -- --run examples/fase69_ninho_campo_operacional_valido.pink
cargo run -- --run examples/fase70_indexacao_array_operacional_valido.pink
cargo run -- --run examples/fase71_cast_memoria_valido.pink
cargo run -- --run examples/fase72_fragil_operacional_minimo_valido.pink
cargo run -- --asm-s examples/fase73_backend_externo_locais_aritmetica_valido.pink
cargo run -- --check examples/fase74_backend_externo_call_minimo_valido.pink
cargo run -- --asm-s examples/fase75_backend_externo_frame_registradores_valido.pink
cargo run -- --asm-s examples/fase75_backend_externo_parametro_nao_bombom_invalido.pink
cargo run -- --asm-s examples/fase76_backend_externo_multiplos_parametros_valido.pink
cargo run -- --asm-s examples/fase77_backend_externo_memoria_frame_valido.pink
cargo run -- --asm-s examples/fase78_backend_externo_composicao_interprocedural_valido.pink
cargo run -- --asm-s examples/fase79_backend_externo_programa_linear_maior_valido.pink
cargo run -- --asm-s examples/fase80_backend_externo_cobertura_linear_ampla_valido.pink
cargo run -- --asm-s examples/fase81_backend_externo_recusa_explicita_tres_parametros_invalido.pink
cargo run -- --asm-s examples/fase82_backend_externo_recusa_explicita_talvez_senao_invalido.pink
cargo run -- --check examples/fase76_backend_externo_tres_args_invalido.pink
cargo run -- --check examples/mut_falho.pink
cargo run -- --check examples/check_quebrar_fora_loop.pink
cargo run -- --check examples/check_continuar_fora_loop.pink
cargo run -- --check examples/check_campo_valido.pink
cargo run -- --check examples/check_indexacao_valida.pink
cargo run -- --check examples/check_indexacao_indice_nao_inteiro.pink
cargo run -- --check examples/check_cast_inteiro_valido.pink
cargo run -- --check examples/fase71_cast_memoria_invalido.pink
cargo run -- --check examples/fase72_fragil_operacional_minimo_invalido.pink
cargo run -- --check examples/check_cast_invalido_logica.pink
cargo run -- --check examples/check_peso_alinhamento_escalar.pink
cargo run -- --check examples/check_peso_alinhamento_array.pink
cargo run -- --check examples/check_peso_alinhamento_ninho.pink
cargo run -- --check examples/check_peso_tipo_inexistente.pink
cargo run -- --check examples/check_volatile_valido.pink
cargo run -- --check examples/check_volatile_invalido.pink
cargo run -- --check examples/check_inline_asm_valido.pink
cargo run -- --check examples/check_inline_asm_multilinha.pink
cargo run -- --check examples/check_inline_asm_invalido_vazio.pink
cargo run -- --check examples/check_freestanding_valido.pink
cargo run -- --check examples/check_freestanding_invalido_fora_topo.pink
cargo run -- --check examples/check_boot_entry_livre_valido.pink
cargo run -- --check examples/check_boot_entry_livre_sem_principal.pink
cargo run -- --check examples/check_kernel_minimo_fase59_valido.pink
cargo run -- --check examples/fase61_verso_valido.pink
cargo run -- --check examples/fase66_deref_seta_u8_invalido.pink
cargo run -- --check examples/fase67_escrita_indireta_seta_u8_invalida.pink
cargo run -- --check examples/fase68_ptr_aritmetica_invalida.pink
cargo run -- --run examples/fase69_ninho_campo_operacional_invalido.pink
cargo run -- --run examples/fase70_indexacao_array_operacional_invalido.pink
cargo run -- --cfg-ir examples/fase61_verso_cfg_ir_invalido.pink
cargo run -- --run examples/fase60_modulos_valido.pink
cargo run -- --check examples/fase60_modulo_ausente.pink
cargo run -- --check examples/fase60_simbolo_ausente.pink
cargo run -- --run examples/fase62_falar_inteiro.pink
cargo run -- --run examples/fase62_falar_logica.pink
cargo run -- --run examples/fase62_falar_verso.pink
cargo run -- --run examples/fase62_falar_expr.pink
cargo run -- build examples/emit_if_else.pink
cargo run -- build --out-dir saida examples/fase60_modulos_valido.pink
```

## Modos da CLI
- `build <arquivo.pink>`: executa pipeline de build e grava artefato `.s` em disco (opcional `--out-dir <dir>`, padrão `build/`)
- `--ir`: IR estruturada (alto nível)
- `--cfg-ir`: CFG IR (blocos, `br`, `jmp`, `ret`)
- `--selected`: camada de seleção de instruções textual (`isel` + `term`)
- `--machine`: alvo textual abstrato de máquina de pilha (`vm` + `term`)
- `--pseudo-asm`: backend textual normalizado final (`ins`/`term`)
- `--asm-s`: backend textual `.s` com ABI textual mínima interna (derivado de `--selected`, sem ABI/registradores finais de plataforma)
- `--run`: interpreta a Machine validada e executa `principal`

## Pipeline de backend textual
`--pseudo-asm` executa:
semântica → IR estruturada → validação da IR estruturada → CFG IR → validação da CFG IR → seleção de instruções → validação da seleção → máquina abstrata → validação da máquina → backend textual → validação do backend textual → impressão.

`--run` executa:
semântica → IR estruturada → validação IR → CFG IR → validação CFG IR → seleção → validação seleção → Machine → validação Machine → interpretação.

Se qualquer camada intermediária for inválida, a emissão falha e nada é impresso.

`--asm-s` executa:
semântica → IR estruturada → validação IR → CFG IR → validação CFG IR → seleção de instruções → validação da seleção → emissão textual `.s` com ABI mínima.

Estado explícito da Fase 54: `--asm-s` cobre o subset escalar (`bombom`, `u8..u64`, `i8..i64`, `logica`, `nulo`) e agora declara contrato textual mínimo de ABI interna (símbolo exportado, `@argN`, `@ret`, prólogo/epílogo textuais). Tipos ainda não suportados seguem falhando de forma clara (ex.: `seta`, `ninho`, arrays).

Estado explícito da Fase 55: além do `--asm-s` textual da Fase 54, existe integração externa **experimental e mínima** para Linux x86_64 via testes (`cc`/`gcc`/`clang`) em subset estrito:
- programa sem globais;
- função única `principal() -> bombom`;
- retorno inteiro constante (`mimo <constante>;` sem instruções intermediárias).

Fluxo experimental reproduzível:
```bash
cargo test --test backend_s_external_toolchain_tests -- --nocapture
```
Se não houver toolchain C no ambiente, o teste de fluxo real é pulado sem quebrar a suíte.

Estado explícito da Fase 73: o subset externo montável foi ampliado (mantendo Linux x86_64 + toolchain C do sistema) para aceitar `principal() -> bombom` com:
- variáveis locais `bombom`;
- atribuição em local;
- aritmética escalar linear (`+`, `-`, `*`);
- retorno calculado a partir de literal/local/temporário.

Limites preservados na Fase 73 (fora do subset externo montável):
- sem globais;
- sem parâmetros;
- sem fluxo de controle (`talvez/senão`, loops);
- sem chamadas de função;
- sem memória indireta/ponteiros no backend externo.

Estado explícito da Fase 74: o subset externo montável ganhou convenção de chamada concreta mínima (ainda Linux x86_64 + toolchain C do sistema) para aceitar chamadas diretas no recorte:
- funções `-> bombom` com bloco único linear;
- `principal() -> bombom` chamando função auxiliar com 0 ou 1 argumento `bombom`;
- passagem de argumento único em `%rdi` e retorno em `%rax`;
- preservando prólogo/epílogo e slots de stack mínimos por função.

Limites preservados na Fase 74 (fora do subset externo montável):
- sem mais de 1 parâmetro;
- sem recursão externa;
- sem fluxo de controle (`talvez/senão`, loops);
- sem globais;
- sem memória indireta/ponteiros no backend externo;
- sem ABI completa de plataforma.

Estado explícito da Fase 75: o subset externo montável manteve o recorte da Fase 74, mas consolidou disciplina mínima de frame/registradores reais para reduzir emissão ad hoc:
- registradores com papel fixo no subset: `%rax` (acumulador/retorno), `%rdi` (argumento único), `%r10` (temporário volátil de binárias);
- frame mínimo explícito por função com `%rbp` e slots lineares para parâmetro/local/temporários, mantendo prólogo/epílogo simples;
- chamadas diretas continuam no subset de até 1 argumento `bombom`, com retorno em `%rax`, sem promover isso a ABI completa.
Limites preservados na Fase 75 (fora do subset externo montável):
- sem mais de 1 parâmetro;
- sem recursão externa;
- sem fluxo de controle (`talvez/senão`, loops);
- sem globais;
- sem memória indireta/ponteiros no backend externo;
- sem register allocation amplo e sem ABI final de plataforma.

Estado explícito da Fase 76: o subset externo montável ampliou a convenção concreta mínima para múltiplos parâmetros reais no recorte Linux x86_64 hospedado:
- chamadas diretas com **até 2 argumentos `bombom`**;
- registradores de argumento no subset: `%rdi` (arg0) e `%rsi` (arg1), com retorno em `%rax`;
- frame mínimo por função preservado com `%rbp`, slots lineares e `%r10` como temporário volátil de binárias;
- suporte mantido para bloco único linear com atribuição, aritmética (`+`, `-`, `*`) e `mimo` calculado.

Limites preservados na Fase 76 (fora do subset externo montável):
- sem 3+ parâmetros;
- sem parâmetros não `bombom`;
- sem recursão externa;
- sem fluxo de controle (`talvez/senão`, loops);
- sem globais;
- sem memória indireta/ponteiros no backend externo;
- sem ABI completa de plataforma e sem register allocation amplo.

Estado explícito da Fase 77: o subset externo montável preserva o recorte de chamadas da Fase 76 e explicita o primeiro recorte de memória real mínima no backend externo:
- load de slot de frame por `%rbp` (`movq -off(%rbp), %reg`);
- store de slot de frame por `%rbp` (`movq %reg, -off(%rbp)`);
- uso desses acessos em fluxo real compilável/montável/linkável/executável no subset Linux x86_64 hospedado.

Limites preservados na Fase 77 (fora do subset externo montável):
- sem memória indireta geral/ponteiros (`*p`, `arr[i]`, `campo` por ponteiro) no backend externo;
- sem globais;
- sem fluxo de controle (`talvez/senão`, loops);
- sem 3+ parâmetros e sem parâmetros não `bombom`;
- sem ABI completa de plataforma e sem register allocation amplo.

Estado explícito da Fase 78: o subset externo montável preserva o recorte da Fase 77 e amplia a composição linear interprocedural no mesmo executável:
- encadeamento linear de chamadas diretas em múltiplos níveis (ex.: `principal -> combina -> ajusta/soma2`);
- passagem de resultados intermediários `bombom` entre funções do subset, com armazenamento em slots de frame quando necessário;
- cobertura externa real (compilar/montar/linkar/executar) para fluxo interprocedural mais rico, sem abrir fundamentos novos de backend.

Limites preservados na Fase 78 (fora do subset externo montável):
- sem controle de fluxo geral (`talvez/senão`, loops) no backend externo;
- sem memória indireta geral/ponteiros no backend externo;
- sem globais, sem 3+ parâmetros e sem parâmetros não `bombom`;
- sem recursão externa e sem ABI completa de plataforma/register allocation amplo.

Estado explícito da Fase 79: o subset externo montável preserva integralmente o recorte da Fase 78 e amplia a densidade do programa linear executável com mais etapas explícitas no mesmo binário:
- fluxo linear mais longo com múltiplas atribuições intermediárias dependentes;
- reuso explícito de locais/temporários de frame antes e depois de chamadas diretas já suportadas;
- composição interprocedural linear mantendo até 2 parâmetros `bombom` e sem abrir novos fundamentos.

Limites preservados na Fase 79 (fora do subset externo montável):
- sem controle de fluxo geral (`talvez/senão`, loops) no backend externo;
- sem memória indireta geral/ponteiros no backend externo;
- sem globais, sem 3+ parâmetros e sem parâmetros não `bombom`;
- sem recursão externa e sem ABI completa de plataforma/register allocation amplo.

Estado explícito da Fase 80: o subset externo montável preserva integralmente o recorte da Fase 79 e amplia a cobertura linear auditável com combinação mais rica de densidade local + composição interprocedural no mesmo executável:
- reuso de chamadas diretas a funções auxiliares com 1 e 2 parâmetros `bombom` em múltiplos trechos lineares do mesmo programa;
- maior reaproveitamento de resultados intermediários em slots de frame entre chamadas sucessivas já suportadas;
- cobertura externa real adicional (compilar/montar/linkar/executar) com validação de resultado observável no Linux x86_64 hospedado.

Limites preservados na Fase 80 (fora do subset externo montável):
- sem controle de fluxo geral (`talvez/senão`, loops) no backend externo;
- sem memória indireta geral/ponteiros no backend externo;
- sem globais, sem 3+ parâmetros e sem parâmetros não `bombom`;
- sem recursão externa e sem ABI completa de plataforma/register allocation amplo.

Estado explícito da Fase 82: o subset externo montável preserva integralmente o recorte da Fase 81 e endurece a fronteira de controle de fluxo por recusa explícita e auditável de `talvez/senão` no caminho `--asm-s` montável.

Limites preservados na Fase 82 (fora do subset externo montável):
- sem controle de fluxo geral (`talvez/senão`, loops) no backend externo;
- sem memória indireta geral/ponteiros no backend externo;
- sem globais, sem 3+ parâmetros e sem parâmetros não `bombom`;
- sem recursão externa e sem ABI completa de plataforma/register allocation amplo.

`--check` continua restrito à validação semântica (não executa lowering IR/CFG nem emissão textual).

Estado explícito da Fase 58: em unidade com `livre;`, `principal() -> bombom` permanece obrigatório e passa a ser tratado como **boot entry mínimo desta fase**, refletido em `--asm-s` como `boot.entry principal -> _start`, junto de um **linker script textual mínimo** (`ENTRY(_start)` + seções básicas). Isso é apenas representação/preparação: não gera kernel bootável real, não integra GRUB/QEMU e não substitui o fluxo hospedado.

Estado explícito da Fase 59: em unidade com `livre;`, `--asm-s` mantém o boot metadata/linker script da Fase 58 e agora também emite um **kernel stub mínimo experimental** (`_start` global chamando `principal` e entrando em loop de parada). O stub é intencionalmente mínimo e auditável, sem prometer boot real universal, GRUB/QEMU/ISO completos ou runtime bare-metal robusto.

## Validação da Machine (sanity check de pilha)
A camada `--machine` agora valida:
- underflow de pilha em instruções/terminadores (`neg`, binárias, `call`, `call_void`, `br_true`)
- consistência de altura de pilha entre predecessores de um bloco
- tipo esperado no topo para `br_true` (condição lógica)
- `ret` com exatamente um valor disponível
- compatibilidade de tipo no `ret` com o retorno da função quando inferível
- aproveitamento de tipos de `params`/`locals` para reduzir `Unknown` em `load_slot`/`store_slot`
- `ret_void` com pilha vazia
- slots válidos por função (`params`, `locals` e temporários `%tN`)

Se a validação estrutural ou de pilha falhar, `--machine` retorna erro e não imprime saída parcial.

Limites atuais (adiado): a tipagem na Machine continua leve/local (sem inferência global pesada entre blocos).

## O que essas camadas representam
- `--cfg-ir`: controle de fluxo explícito próximo do lowering
- `--selected`: instruções selecionadas e terminadores já disciplinados
- `--machine`: alvo textual abstrato de execução (pilha + controle de fluxo)
- `--pseudo-asm`: formato textual final estável para auditoria e golden tests

## O que ainda não representam
- não são assembly real de CPU
- não são backend executável
- não fazem otimizações ou alocação de registradores

## Documentação do projeto

O projeto mantém um ecossistema documental com papéis distintos:

- `docs/roadmap.md` — trilha ativa oficial; define a ordem funcional em execução.
- `docs/future.md` — inventário técnico amplo de médio/longo prazo; não dita ordem ativa.
- `docs/parallel.md` — documento visionário; guarda identidade e direção conceitual; não é backlog técnico.
- `docs/history.md` — crônica histórica oficial (fases funcionais, hotfixes e rodadas documentais).
- `docs/agent_state.md` — estado corrente enxuto + diretrizes operacionais consolidadas.
- `docs/handoff_codex.md` — handoff operacional curto da rodada atual.
- `docs/doc_rules.md` — convenções obrigatórias de atualização e precedência documental.

## Licença
[MIT](LICENSE)
