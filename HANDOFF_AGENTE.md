# Handoff Técnico: Pinker v0

Este documento existe para permitir que outra pessoa ou outro agente de IA abra este repositório e entenda, com precisão, o que já foi feito, o que está congelado, como o projeto funciona hoje e onde mexer sem quebrar o frontend.

O objetivo aqui não é vender a linguagem nem discutir roadmap amplo. O objetivo é transferir contexto operacional real.

## 1. O que este projeto é hoje

O projeto `pinker-v0` é um frontend pequeno, auditável e estabilizado, escrito em Rust, para uma linguagem chamada Pinker.

Hoje ele faz:
- leitura de arquivo fonte `.pink`
- análise léxica
- parsing para AST
- impressão textual da AST
- serialização manual da AST em JSON
- checagem semântica básica
- lowering para uma IR textual própria e pequena
- impressão textual da IR

Hoje ele não faz:
- codegen
- backend nativo
- LLVM
- Cranelift
- otimização
- SSA
- FFI
- ponteiros
- structs
- enums
- generics
- traits

Em outras palavras: este projeto é um frontend congelado e confiável, já com uma primeira IR textual interna, mas ainda sem qualquer backend.

## 2. Escopo congelado da linguagem

A gramática e a semântica da v0 devem ser tratadas como congeladas, salvo correção de bug real.

### 2.1. Construções suportadas

- `pacote`
- `carinho`
- `mimo`
- `talvez` / `senao`
- `eterno`
- `nova`
- `mut`
- tipos `bombom` e `logica`
- literais `verdade`, `falso`, inteiros
- chamadas diretas de função por nome
- expressões unárias e binárias

### 2.2. Regras semânticas já consolidadas

- deve existir uma função `principal`
- `principal` não pode receber parâmetros
- `principal` deve retornar `bombom`
- variáveis locais só podem ser reatribuídas se forem `mut`
- chamadas verificam existência, aridade e tipo dos argumentos
- função sem tipo de retorno explícito é tratada internamente como `Nulo`
- `mimo;` é válido só em função sem retorno declarado
- `mimo valor;` é inválido em função sem retorno declarado
- `mimo;` é inválido em função com retorno declarado
- uso de função sem retorno em contexto de valor é erro semântico
- fluxo de retorno é verificado apenas para casos simples tratáveis

### 2.3. Simplificações deliberadas

Estas simplificações são intencionais, não bugs:
- análise de fluxo não é completa
- a IR não é SSA
- `talvez` continua estruturado na IR
- chamadas indiretas não existem
- `Nulo` não é tipo de superfície da linguagem; existe apenas internamente

## 3. Árvore do projeto e papel de cada arquivo

### 3.1. Raiz

- `Cargo.toml`
  - manifesto do projeto
  - o projeto é um binário/aplicação, por isso `Cargo.lock` é mantido

- `Cargo.lock`
  - lockfile do binário

- `.gitignore`
  - ignora `target/`, temporários e artefatos locais

- `README.md`
  - resumo técnico curto do projeto e da CLI

- `HANDOFF_AGENTE.md`
  - este documento

### 3.2. Código em `src/`

- `src/lib.rs`
  - expõe os módulos públicos do frontend

- `src/main.rs`
  - CLI
  - pipeline do executável
  - parse de flags
  - ordem das etapas

- `src/token.rs`
  - definição de `TokenKind`, `Token`, `Position`, `Span`
  - utilidades de nome e exibição

- `src/lexer.rs`
  - transforma fonte em vetor de tokens
  - gera spans léxicos
  - trata comentários de linha

- `src/ast.rs`
  - AST da Pinker v0
  - tipo interno `Nulo`
  - serializer manual do JSON AST

- `src/parser.rs`
  - parser descendente recursivo/precedência
  - constrói AST
  - controla spans sintáticos

- `src/error.rs`
  - tipo central de erro
  - variantes léxica, sintática, expected/found, semântica e IR

- `src/semantic.rs`
  - checagem semântica
  - tabela de funções/constantes
  - escopos locais
  - política de `principal`
  - política de retorno
  - política de `Nulo`

- `src/printer.rs`
  - printer textual da AST
  - `render_program`
  - `render_program_json` delega ao serializer da AST

- `src/ir.rs`
  - primeira IR textual própria do projeto
  - estruturas internas da IR
  - lowering AST -> IR
  - printer textual da IR

### 3.3. Testes em `tests/`

- `tests/common/mod.rs`
  - helpers compartilhados
  - tokenização, parsing, semântica, render de AST/JSON/IR

- `tests/lexer_tests.rs`
  - tokens, spans léxicos, formato de erro léxico

- `tests/parser_tests.rs`
  - parsing de função, `if/else`, atribuição, spans sintáticos

- `tests/semantic_tests.rs`
  - principal, retorno, mutabilidade, chamadas, `Nulo`, erros semânticos

- `tests/output_tests.rs`
  - golden tests da AST textual
  - golden tests do JSON AST
  - casos com arrays múltiplos para validar vírgulas no JSON

- `tests/ir_tests.rs`
  - lowering para IR
  - formato textual da IR
  - golden tests da IR

### 3.4. Exemplos em `examples/`

Há exemplos gerais do frontend e exemplos específicos da IR.

Exemplos gerais:
- `principal_valida.pink`
- `principal_invalida.pink`
- `retorno_ok.pink`
- `retorno_falho.pink`
- `mut_ok.pink`
- `mut_falho.pink`
- `if_logico_ok.pink`
- `if_logico_falho.pink`
- `chamada_args_ok.pink`
- `chamada_args_falho.pink`
- `nulo_e_void.pink`
- `json_ast_demo.pink`

Exemplos específicos da IR:
- `ir_funcao_simples.pink`
- `ir_if_else.pink`
- `ir_chamada.pink`
- `ir_nulo.pink`

## 4. Pipeline real do executável

O pipeline do binário é:

1. ler arquivo fonte
2. executar lexer
3. opcionalmente imprimir `--tokens`
4. executar parser
5. opcionalmente imprimir `--ast`
6. opcionalmente imprimir `--json-ast`
7. executar semântica
8. se `--ir`, fazer lowering AST -> IR
9. imprimir IR textual
10. imprimir mensagem final de sucesso, exceto em `--check`

### 4.1. Ordem importante

`--ir` só roda depois da semântica.

Isso é importante porque o lowering assume:
- funções registradas corretamente
- chamadas já semanticamente válidas
- uso de `Nulo` já resolvido pela semântica
- `principal` já validada

Se você tentar pular a semântica antes do lowering, estará mudando uma premissa de arquitetura do projeto.

## 5. Formatos de saída já existentes

### 5.1. `--tokens`

Imprime tokens com:
- nome estável do token
- lexema
- span

Exemplo:

```text
KwPacote 'pacote' [1:1..1:7]
```

### 5.2. `--ast`

É um formato textual humano-legível.

Ele:
- não é JSON
- não deve ser tratado como API de máquina
- é útil para inspeção visual e testes de golden output

### 5.3. `--json-ast`

É JSON manualmente serializado, com formato estável.

Política importante:
- ausência de tipo de retorno no AST JSON => `ret_type: null`
- `ReturnStmt` sem expressão => `expr: null`

Isso é intencional e hoje é parte da compatibilidade de saída.

### 5.4. `--ir`

É a impressão textual da IR.

Ele difere de `--ast` porque:
- já não é a árvore sintática literal
- resolve locais e constantes em uma estrutura intermediária
- mostra blocos e instruções de uma forma mais próxima de execução futura

Ele difere de `--json-ast` porque:
- não representa a AST
- não é JSON
- não tenta preservar o formato sintático original do programa

## 6. Modelo da AST

A AST é intencionalmente pequena.

Principais nós:
- `Program`
- `PackageDecl`
- `Item`
  - `Function`
  - `Const`
- `FunctionDecl`
- `ConstDecl`
- `Block`
- `Stmt`
  - `Let`
  - `Return`
  - `Assign`
  - `If`
  - `Expr`
- `Expr`
  - `Binary`
  - `Unary`
  - `Call`
  - `Ident`
  - `IntLit`
  - `BoolLit`

Tipos:
- `Bombom`
- `Logica`
- `Nulo`

`Nulo` é interno. Não invente sintaxe pública para ele a menos que a linguagem inteira seja deliberadamente expandida.

## 7. Modelo da semântica

### 7.1. Registro em duas camadas

Primeiro a semântica registra:
- funções
- constantes globais

Depois verifica:
- `principal`
- corpos de funções
- inicializadores de constantes

### 7.2. Escopos

Escopos são pilha de mapas.

Busca de variável:
- do escopo mais interno para o mais externo
- constantes globais entram como fallback somente quando nenhuma variável local cobre o nome

### 7.3. Retorno

A política de retorno já está consolidada. Evite reabrir isso sem motivo forte.

Regra prática:
- função com tipo de retorno declarado deve retornar em todos os caminhos simples
- `talvez` sem `senao` não fecha exaustividade sozinho

## 8. Modelo da IR

### 8.1. O que ela é

A IR atual é estruturada.

Não há:
- blocos básicos com jump explícito
- phi
- registradores SSA

Há:
- módulo
- constantes globais
- funções
- bloco `entry`
- blocos aninhados em `If`
- instruções explícitas

### 8.2. Tipos da IR

Estruturas principais:
- `ProgramIR`
- `ConstIR`
- `FunctionIR`
- `BindingIR`
- `LocalIR`
- `BlockIR`
- `InstructionIR`
- `ValueIR`
- `TypeIR`

### 8.3. Instruções da IR

- `Let`
- `Assign`
- `Expr`
- `Return`
- `If`

### 8.4. Valores da IR

- `Local`
- `GlobalConst`
- `Int`
- `Bool`
- `Unary`
- `Binary`
- `Call`

### 8.5. Política de variáveis na IR

Variáveis locais são lowered como slots nomeados e estáveis:
- `%x#0`
- `%x#1`

Isso resolve shadowing de forma auditável sem precisar de SSA.

Parâmetros:
- também ganham slots
- aparecem em `params`
- não aparecem em `locals`

Locais:
- só declarações `nova`
- guardam tipo e mutabilidade

### 8.6. Política de `Nulo` na IR

`Nulo` aparece como `TypeIR::Nulo`.

Casos típicos:
- função sem retorno declarado => `func log -> nulo`
- chamada de função sem retorno => `call log() -> nulo`
- `return` vazio => `return` sem operando

Ou seja:
- `Nulo` existe como tipo da IR
- mas retorno vazio ainda é modelado como ausência de valor no `Return`

### 8.7. Política de `talvez` na IR

`talvez` vira `InstructionIR::If` com:
- condição
- `then_block`
- `else_block` opcional

`else if` continua estruturado e é lowered como bloco `else_*` contendo outro `If`.

Isso é simples e intencional.

## 9. Serializer manual do JSON AST

Este é um ponto historicamente sensível.

O projeto usa serializer manual, não `serde`.

Razão:
- zero-dependency
- auditabilidade
- saída totalmente controlada

### 9.1. Bug já corrigido

Houve um bug em arrays com múltiplos elementos:
- faltavam vírgulas entre objetos consecutivos
- isso afetava `items`, `params`, `stmts`, `args`

Esse bug foi corrigido no `JsonWriter`.

Se você tocar no serializer:
- rode obrigatoriamente `tests/output_tests.rs`
- valide arrays com mais de um elemento
- não quebre a política de `null`

## 10. Testes: como interpretar

### 10.1. Filosofia

Os testes são a especificação executável mais confiável do projeto.

Se você tiver dúvida entre:
- uma lembrança informal
- um comentário antigo
- um teste atual que passa

prefira o teste atual que passa.

### 10.2. Golden tests

Os golden tests não existem por estética; eles existem para congelar formato.

Isso vale para:
- AST textual
- JSON AST
- IR textual

Se mudar o formato de saída:
- só faça isso com justificativa explícita
- ajuste os testes conscientemente
- documente a mudança

### 10.3. O que não fazer

Não troque asserts exatos por `contains` fraco só para “fazer passar”.

Nos pontos críticos:
- JSON AST
- IR
- formato de erro

o projeto quer previsibilidade, não flexibilidade frouxa.

## 11. Convenções práticas ao mexer aqui

### 11.1. Se for mexer em AST/JSON

Sempre validar:
- `cargo test --test output_tests`
- `cargo test`

### 11.2. Se for mexer em semântica

Sempre validar:
- `cargo test --test semantic_tests`
- `cargo test --test ir_tests`
- `cargo test`

A IR depende das garantias da semântica.

### 11.3. Se for mexer no parser

Sempre validar:
- `cargo test --test parser_tests`
- `cargo test --test output_tests`
- `cargo test --test semantic_tests`
- `cargo test`

### 11.4. Se for mexer na IR

Sempre validar:
- `cargo test --test ir_tests`
- `cargo test`

E, se mexer no lowering:
- verifique se AST e semântica continuam intocadas

## 12. Comandos úteis

### 12.1. Build

```bash
cd /home/aylavictoria/Área\ de\ trabalho/língua/pinker-v0
cargo build
```

### 12.2. Testes completos

```bash
cargo test
```

### 12.3. Executar com tokens

```bash
cargo run -- --tokens examples/principal_valida.pink
```

### 12.4. Executar com AST textual

```bash
cargo run -- --ast examples/json_ast_demo.pink
```

### 12.5. Executar com JSON AST

```bash
cargo run -- --json-ast examples/json_ast_demo.pink
```

### 12.6. Executar com IR

```bash
cargo run -- --ir examples/ir_if_else.pink
```

### 12.7. Verificação semântica silenciosa

```bash
cargo run -- --check examples/mut_falho.pink
```

## 13. Invariantes que não devem ser quebradas sem decisão explícita

- `principal` continua obrigatória
- `principal` continua sem parâmetros
- `principal` continua retornando `bombom`
- AST textual continua legível e estável
- JSON AST continua manual, estável e válido
- IR continua separada da AST
- `--ir` só roda após semântica válida
- `ret_type` ausente no JSON AST continua sendo `null`
- `expr` ausente em `ReturnStmt` continua sendo `null`
- frontend não deve começar a crescer para features novas sem intenção explícita

## 14. Locais prováveis de erro se alguém mexer sem cuidado

### 14.1. Spans

Qualquer ajuste de span em:
- lexer
- parser
- merge de blocos
- wrapping de expressões

pode quebrar muitos golden tests e mensagens.

### 14.2. JSON AST

O serializer manual pode parecer simples, mas arrays/objetos e estado de vírgula são fáceis de quebrar.

### 14.3. Lowering da IR

O lowering depende de:
- nomes de função já registrados
- escopos corretos
- shadowing resolvido por slot

Se você mudar a política de binding, os golden tests da IR vão precisar ser revisados.

### 14.4. `Nulo`

`Nulo` toca:
- semântica
- JSON AST
- IR
- testes

É um ponto pequeno, mas transversal.

## 15. O que um próximo agente deve fazer primeiro ao abrir este projeto

Ordem recomendada:

1. ler `README.md`
2. ler este `HANDOFF_AGENTE.md`
3. rodar `cargo test`
4. olhar `src/main.rs` para entender o pipeline
5. olhar `src/semantic.rs` para entender as garantias
6. olhar `src/ir.rs` para entender a fase atual
7. só então começar a editar

## 16. O que NÃO fazer em um primeiro contato

- não expandir a linguagem “porque parece fácil”
- não trocar o serializer manual por dependência externa sem decisão clara
- não refatorar agressivamente tudo de uma vez
- não mudar formato de saída sem atualizar os golden tests conscientemente
- não tentar acoplar backend agora
- não mexer na IR como se ela já fosse LLVM-lite

## 17. Estado resumido, em uma frase

Este repositório é um frontend Pinker v0 congelado, confiável e testado, com AST, JSON AST, semântica e uma primeira IR textual estruturada, pronto para evolução incremental cuidadosa, mas não para expansão ampla descontrolada.
