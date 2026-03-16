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
- IR textual propria e estruturada

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

## Congelamento da v0
- entra na v0: frontend atual, spans, diagnosticos, printer textual, JSON AST, exemplos e testes
- nao entra na v0: expansao da linguagem
- simplificacao deliberada: fluxo de retorno apenas para casos simples trataveis
- adiado: qualquer backend e qualquer feature fora do frontend existente

## Build e testes
```bash
cd /home/aylavictoria/Área\ de\ trabalho/língua/pinker-v0
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
cargo run -- --check examples/mut_falho.pink
```

## Modos da CLI
- `--tokens`: imprime tokens com spans
- `--ast`: imprime AST textual legivel
- `--json-ast`: imprime AST JSON valido
- `--ir`: imprime a IR textual depois de parsing + semantica
- `--check`: executa so a validacao semantica; em sucesso nao imprime AST nem tokens

## Formato de erro
- lexico: `Erro Léxico: <mensagem> em <inicio>..<fim>`
- sintatico: `Erro Sintático: <mensagem> em <inicio>..<fim>`
- expected/found: `Erro Sintático: esperado '<x>', encontrado '<y>' em <inicio>..<fim>`
- semantico: `Erro Semântico: <mensagem> em <inicio>..<fim>`
- IR: `Erro IR: <mensagem> em <inicio>..<fim>`

## O que a IR representa
- programa apos parsing e semantica bem-sucedidos
- funcoes, blocos, constantes globais, locais, atribuicoes, retornos, chamadas e `talvez`
- estrutura propria, separada da AST textual

## O que a IR ainda nao e
- nao e SSA
- nao e LLVM
- nao tem otimizacao
- nao faz codegen

## Saidas curtas
```text
$ cargo run -- examples/principal_valida.pink
Análise semântica concluída sem erros.
```

```text
$ cargo run -- --tokens examples/principal_valida.pink
=== TOKENS ===
KwPacote 'pacote' [1:1..1:7]
Ident 'main' [1:8..1:12]
...
```

```text
$ cargo run -- --check examples/mut_falho.pink
Erro Semântico: reatribuição inválida: 'x' não é mutável em 5:5..5:12
```

```text
$ cargo run -- --ir examples/ir_funcao_simples.pink
=== IR ===
module main
consts:
  []
functions:
  func principal -> bombom
    params: []
    locals: []
    block entry:
      return 0:bombom
Análise semântica concluída sem erros.
```
