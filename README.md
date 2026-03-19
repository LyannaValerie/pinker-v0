# Pinker v0

Pinker v0 é um frontend pequeno e congelado em Rust para a linguagem Pinker.

## O que o frontend faz hoje
- léxico com spans
- parser para `pacote`, `carinho`, `mimo`, `talvez/senão`, `sempre que`, `eterno`, `nova`, `mut`
- tipos `bombom`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64` e `logica`
- aliases de tipo (`apelido`), arrays fixos (`[tipo; N]`), structs (`ninho`), ponteiros (`seta<tipo>`)
- qualificador `fragil` (`volatile`) para ponteiros explícitos (`fragil seta<tipo>`)
- cast explícito controlado com `virar` (inteiro -> inteiro no frontend/semântica/IR estruturada)
- consultas estáticas de layout com `peso(tipo)` e `alinhamento(tipo)`
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

## O que não faz
- codegen nativo real
- backend nativo
- LLVM / Cranelift
- otimizações grandes
- FFI, enums, generics, traits
- operações reais de ponteiro (dereferência, aritmética), acesso via ponteiro (`seta<T>`), escrita em campo/index, layout físico/ABI
- semântica operacional de `fragil` em runtime/backend (nesta fase é qualificador semântico preservado no pipeline)
- lowering operacional de `virar` em CFG/Machine/runtime (`--check` aceita o subset da fase; `--run`/`--cfg-ir` ainda não executam cast)
- runtime signed correto (tipos `i8`–`i64` são bloqueados no `--run` até representação adequada)

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
cargo run -- --run examples/run_alias_tipo_basico.pink
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
```

## Modos da CLI
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
