# Pinker v0

Pinker v0 e um frontend pequeno e congelado em Rust para a linguagem Pinker.

## O que o frontend faz hoje
- lexico com spans
- parser para `pacote`, `carinho`, `mimo`, `talvez/senao`, `eterno`, `nova`, `mut`
- tipos `bombom` e `logica`
- chamadas diretas por nome
- checagem semantica de `principal`, retorno, mutabilidade, aridade e tipos
- AST textual estavel
- AST JSON estavel
- IR estruturada + validacao interna
- CFG IR + validacao interna
- seleção de instruções textual + validação
- alvo textual abstrato (máquina de pilha) + validação
- backend textual pseudo-assembly + validacao interna

## O que nao faz
- codegen nativo real
- backend nativo
- LLVM / Cranelift
- otimizações grandes
- FFI, ponteiros, structs, enums, generics, traits

## Build e testes
```bash
cargo build
cargo test
```

## Uso
```bash
cargo run -- examples/principal_valida.pink
cargo run -- --ir examples/ir_if_else.pink
cargo run -- --cfg-ir examples/cfg_if_else.pink
cargo run -- --selected examples/selected_if_else.pink
cargo run -- --machine examples/machine_if_else.pink
cargo run -- --pseudo-asm examples/emit_if_else.pink
cargo run -- --check examples/mut_falho.pink
```

## Modos da CLI
- `--ir`: IR estruturada (alto nivel)
- `--cfg-ir`: CFG IR (blocos, `br`, `jmp`, `ret`)
- `--selected`: camada de seleção de instruções textual (`isel` + `term`)
- `--machine`: alvo textual abstrato de máquina de pilha (`vm` + `term`)
- `--pseudo-asm`: backend textual normalizado final (`ins`/`term`)

## Pipeline de backend textual
`--pseudo-asm` executa:
semantica -> IR estruturada -> validação da IR estruturada -> CFG IR -> validação da CFG IR -> seleção de instruções -> validação da seleção -> máquina abstrata -> validação da máquina -> backend textual -> validação do backend textual -> impressão.

Se qualquer camada intermediária for inválida, a emissão falha e nada é impresso.

`--check` continua restrito à validação semântica (não executa lowering IR/CFG nem emissão textual).

## O que essas camadas representam
- `--cfg-ir`: controle de fluxo explícito próximo do lowering
- `--selected`: instruções selecionadas e terminadores já disciplinados
- `--machine`: alvo textual abstrato de execução (pilha + controle de fluxo)
- `--pseudo-asm`: formato textual final estável para auditoria e golden tests

## O que ainda não representam
- não são assembly real de CPU
- não são backend executável
- não fazem otimizações ou alocação de registradores
