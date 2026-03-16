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
cargo run -- --pseudo-asm examples/emit_if_else.pink
cargo run -- --check examples/mut_falho.pink
```

## Modos da CLI
- `--ir`: IR estruturada (alto nivel)
- `--cfg-ir`: CFG IR (blocos, `br`, `jmp`, `ret`)
- `--pseudo-asm`: backend textual normalizado (`ins`/`term`) emitido da CFG IR validada

## Pipeline de backend textual
`--pseudo-asm` executa:
semantica -> IR estruturada -> validação da IR estruturada -> CFG IR -> validação da CFG IR -> backend textual -> validação do backend textual -> impressão.

Se a CFG IR ou o backend textual forem inválidos, a emissão falha e nada é impresso.

`--check` continua restrito à validação semântica (não executa lowering IR/CFG nem emissão textual).

## O que o backend textual representa
- formato textual estável para auditoria e golden tests
- separação explícita entre instruções (`ins`) e terminador de bloco (`term`)
- ponte simples entre CFG IR e backend real futuro

## O que ainda não representa
- não é assembly real de CPU
- não é backend executável
- não faz otimizações ou alocação de registradores
