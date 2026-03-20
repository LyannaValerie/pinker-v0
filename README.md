# Pinker v0

Pinker v0 é um frontend pequeno e congelado em Rust para a linguagem Pinker.

## O que o frontend faz hoje
- léxico com spans
- parser para `pacote`, `trazer`, `carinho`, `mimo`, `talvez/senão`, `sempre que`, `eterno`, `nova`, `mut`
- tipos `bombom`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` e `logica`
- aliases de tipo (`apelido`), arrays fixos (`[tipo; N]`), structs (`ninho`), ponteiros (`seta<tipo>`)
- representação mínima de ponteiro no runtime (`RuntimeValue::Ptr`) para `seta<T>` no `--run`
- qualificador `fragil` (`volatile`) para ponteiros explícitos (`fragil seta<tipo>`)
- inline asm mínimo como statement textual com `sussurro("...")` (ou múltiplas strings), preservado até IR
- marca de unidade freestanding/no-std com `livre;` no topo do programa
- cast explícito controlado com `virar` (inteiro -> inteiro no frontend/semântica/IR estruturada)
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
- operações reais de ponteiro (dereferência, aritmética), acesso via ponteiro (`seta<T>`), escrita em campo/index, layout físico/ABI
- leitura/escrita indireta por ponteiro e aritmética de ponteiro no runtime (apenas representação mínima existe nesta fase)
- semântica operacional de `fragil` em runtime/backend (nesta fase é qualificador semântico preservado no pipeline)
- lowering operacional de `virar` em CFG/Machine/runtime (`--check` aceita o subset da fase; `--run`/`--cfg-ir` ainda não executam cast)
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
cargo run -- --check examples/mut_falho.pink
cargo run -- --check examples/check_quebrar_fora_loop.pink
cargo run -- --check examples/check_continuar_fora_loop.pink
cargo run -- --check examples/check_campo_valido.pink
cargo run -- --check examples/check_indexacao_valida.pink
cargo run -- --check examples/check_indexacao_indice_nao_inteiro.pink
cargo run -- --check examples/check_cast_inteiro_valido.pink
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

## Backlog futuro
- Visão estruturada de evolução de longo prazo em `docs/future.md` (não é roadmap rígido de curto prazo).

## Licença
[MIT](LICENSE)
