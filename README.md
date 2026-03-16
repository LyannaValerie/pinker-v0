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
- IR estruturada (alto nivel)
- validacao interna da IR estruturada
- CFG IR (blocos rotulados com saltos explicitos)

## O que nao faz
- codegen
- backend nativo
- LLVM
- FFI
- ponteiros
- structs
- enums
- generics
- traits

## Build e testes
```bash
cargo build
cargo test
```

## Uso
```bash
cargo run -- examples/principal_valida.pink
cargo run -- --tokens examples/principal_valida.pink
cargo run -- --ast examples/json_ast_demo.pink
cargo run -- --json-ast examples/json_ast_demo.pink
cargo run -- --ir examples/ir_if_else.pink
cargo run -- --cfg-ir examples/cfg_if_else.pink
cargo run -- --check examples/mut_falho.pink
```

## Modos da CLI
- `--tokens`: imprime tokens com spans
- `--ast`: imprime AST textual legivel
- `--json-ast`: imprime AST JSON valido
- `--ir`: imprime a IR estruturada apos parsing + semantica
- `--cfg-ir`: imprime a IR baixa em CFG com blocos e terminadores explicitos
- `--check`: executa so a validacao semantica

## Duas formas de IR
- `--ir`: IR estruturada, mais proxima da estrutura de alto nivel (`if` estruturado)
- `--cfg-ir`: IR baixa com `block`, `br`, `jmp` e `ret` explicitos

A CFG IR ainda nao e SSA, nao otimiza e nao gera codigo nativo.
