# Pinker v0

Pinker v0 é a base factual atual da linguagem Pinker: um compilador/frontend em Rust,
com interpretador, IRs auditáveis, backend nativo próprio e documentação versionada.

Este README é a porta de entrada. Ele não tenta ser manual completo nem histórico de
fases; para isso, use os documentos apontados em [Navegação](#navegacao).

## Estado Atual

| Área | Estado |
|---|---|
| Linguagem | Frontend implementado para o contrato versionado atual |
| Runtime interpretado | `pink --run` executa a superfície estável do workspace |
| Backend nativo | `pink build --nativo` gera ELF Linux via `.s` x86-64 System V + `pinker_rt` |
| Paridade | Fases compatíveis do Eixo B verificam interpretador x nativo |
| Bloco ativo | Bloco 20: expansão rumo a SO e self-hosting |
| Fase funcional mais recente | Fase 248: uniões estruturais tagged |

## Superficie Implementada

| Frente | O que existe hoje |
|---|---|
| Base sintática | `pacote`, `trazer`, `carinho`, `mimo`, `nova`, `muda`, `talvez/senão`, laços e blocos |
| Tipos escalares | `bombom`, inteiros com largura/sinal, `logica`, `verso` |
| Dados compostos | `ninho`, arrays fixos, `leque`, `uniao<T...>`, `lista<T>` versionada e `mapa<K,V>` nas combinações públicas |
| Resultado | `leque` com carga, `encaixe`, `tentar`, `propagar` e `propagar?`; `Resultado<T,E>` é declarado pelo programa ou por módulo importado, não um tipo predeclarado mágico |
| Generics | `lista<T>`, `mapa<K,V>`, `leque<T...>` via alias explícito e funções genéricas explícitas `nome<T>(...)` com monomorfização |
| Contratos | `trato`/`impl` estáticos, `trato<Nome>` explícito, materialização por `virar`, vtables e despacho dinâmico nativo |
| Funções | `carinho`, callables/closures de alto nível e ponteiros crus `seta<carinho(...) -> T>` com chamada indireta sem ambiente |
| Sistema | argv, ambiente, arquivos por descritor, processos com resolução determinística, caminhos e texto no recorte versionado |
| Ponteiros e memória | `seta<T>`, `&funcao`/`&generica<T>`, deref/escrita indireta escalar, aritmética tipada e regiões públicas `alocar(u64)`/`liberar(seta<u8>)` com validação fail-closed, proveniência e orçamento explícito |
| Baixo nível | `sussurro("...")` emite assembly GNU Intel x86-64 no backend nativo; o interpretador rejeita sua execução deterministicamente |
| Ferramentas | CLI com check, run, IR textual, CFG, machine e build nativo |

## Limites Honestos

Pinker v0 ainda não é uma linguagem geral, nem um compilador de produção.

| Fora do recorte atual | Observação |
|---|---|
| LLVM, Cranelift, JIT e otimizações globais | O backend atual é próprio, simples e auditável |
| Multi-plataforma, múltiplas ABIs e bare-metal real | O alvo nativo atual é ELF Linux x86-64 System V |
| Runtime em Pinker | `pinker_rt` ainda vive no workspace Rust/C ABI |
| Ownership/lifetime de objetos de trato | Handles, snapshots e descritores não são liberados nem coletados neste recorte |
| Ownership geral da memória | `alocar`/`liberar` é mecanismo explícito e limitado; não há GC, RAII, borrow checker nem garantia universal para ponteiros raw |
| Default methods, downcasting/upcasting, herança e objetos de múltiplos tratos | Fora do contrato da Fase 244 |
| Generics amplos em `ninho`/`leque` e inferência genérica | O recorte atual exige chamada genérica explícita |
| Ponteiros e layout físico completos | Há operações úteis, mas ainda conservadoras |
| Profundidade de chamadas | O interpretador limita a pilha Pinker a 64 chamadas simultâneas (`MAX_CALL_DEPTH = 64`) e diagnostica a 65ª |
| Uniões estruturais | Valores são handles de uma palavra com lifetime monotônico; igualdade, hashing, serialização, `falar` direto e downcast fora de `encaixe` ficam fora desta fase |
| Biblioteca padrão rica | APIs existem por fases e recortes objetivos |
| SO em Pinker | A cadeia freestanding foi formalizada no roadmap, mas nenhuma capacidade bare-metal foi implementada por essa decisão documental |

## Fluxo Rápido

```bash
make ci
make run-example EX=examples/principal_valida.pink
make check-example EX=examples/principal_valida.pink
make audit-example EX=examples/principal_valida.pink
```

Sem `make`:

```bash
./ci_env.sh --preflight
./ci_env.sh cargo build --locked
./ci_env.sh cargo test --locked
./ci_env.sh cargo fmt --check
./ci_env.sh cargo clippy --all-targets --all-features -- -D warnings
```

## CLI

No checkout, o caminho oficial mais curto para descobrir a CLI é:

```bash
./ci_env.sh cargo run --bin pink -- --help
```

Depois de `make build` ou `./ci_env.sh cargo build --locked`, o mesmo contrato
pode ser verificado diretamente com `./target/debug/pink --help`. Ajuda
solicitada escreve em stdout e termina com código `0`; diagnósticos de uso
inválido escrevem em stderr e terminam com código `2`.

| Comando | Uso |
|---|---|
| `pink --help`, `pink -h`, `pink help` | Ajuda principal |
| `pink help COMANDO` | Ajuda de `build`, `editor`, `repl`, `doc`, `nav`, `agente` ou `estado` |
| `pink COMANDO --help`, `pink COMANDO -h` | Forma equivalente de ajuda do comando |
| `pink --version`, `pink -V` | Versão determinística do pacote |
| `pink arquivo.pink` | Compila/checka o arquivo pelo caminho padrão |
| `pink --check arquivo.pink` | Validação sem execução |
| `pink --run arquivo.pink` | Execução interpretada |
| `pink --ir arquivo.pink` | Emissão de IR textual |
| `pink --cfg-ir arquivo.pink` | Emissão de CFG IR |
| `pink --machine arquivo.pink` | Emissão da machine abstrata |
| `pink build --nativo arquivo.pink` | Geração de executável nativo no recorte suportado |
| `pink nav projecao listar` | Inventário dos snapshots históricos de navegação |
| `pink nav projecao mostrar ID [--observado]` | Definição canônica e observação separadas |
| `pink nav projecao verificar [ID]` | Verificação composta, com drift e harness distintos |
| `pink nav projecao preparar ID ...` | Planeja/prepara CANDIDATE por digest explícito |
| `pink nav projecao aceitar ID ...` | Planeja/aceita a transição única para FROZEN |
| `pink nav localizar SÍMBOLO [--json]` | Resolve símbolos e vínculos explícitos pelo índice derivado |
| `git diff --unified=0 \| pink nav cobertura-diff [--json]` | Relaciona arquivos e linhas a regiões, docs, projeções e testes explícitos |
| `pink estado [--repo DIRETÓRIO] [--agente-spec ARQUIVO] [--json]` | Estado consolidado, determinístico e somente leitura do projeto |

Os códigos públicos distinguem `0` (sucesso, ajuda ou versão), `1` (falha
operacional genérica) e `2` (invocação inválida). `doc` e `nav` preservam os
códigos estruturados `3` para catálogo ausente ou inválido, `4` para ausência
de resultado e `5` para fonte ou âncora divergente; `agente` preserva seus
códigos operacionais depois do parsing. `nav projecao` acrescenta `6` para
falha de harness, `7` para violação de política e `8` para plano obsoleto,
preservando `5` para drift. A versão humana contém apenas
`pink` e a versão do pacote: commit de build e versões de schema ficam
reservados a uma possível saída estruturada futura, ainda sem contrato nesta
etapa.

`pink estado` separa o estado observado do sucesso da consulta: um relatório
`WARNING`, `BLOCKED` ou `PARTIAL` termina com `0`; `1` é falha interna, `2` é
uso inválido e `3` indica que nenhuma raiz mínima pôde ser estabelecida.

Os comandos raiz existentes são somente `build`, `editor`, `repl`, `doc`,
`nav`, `agente` e `estado`. Propostas como `pink comandos`, `pink env`, `pink doctor` e
`pink listar` não estão implementadas; `listar` existe apenas dentro das
gramáticas próprias de `doc` e `nav`.

## Exemplo Minimo

```pinker
pacote exemplo;

carinho dobro(x: bombom) -> bombom {
    retornar x + 2;
}

carinho principal() -> bombom {
    falar("dobro", dobro(40));
    retornar 0;
}
```

```bash
./ci_env.sh cargo run --bin pink -- --run examples/principal_valida.pink
```

## Pipeline

```text
fonte .pink
  -> lexer/parser com spans
  -> AST tipada e validada
  -> IR textual / CFG IR
  -> machine abstrata
  -> interpretador ou backend .s
  -> runtime pinker_rt no caminho nativo
```

Detalhes de arquitetura vivem em `docs/code_map.md`, `docs/atlas.md` e nos testes de
pipeline. O README só mantém o mapa de leitura.

## Navegacao

| Documento | Papel |
|---|---|
| [Começar a contribuir](CONTRIBUTING.md) | Porta de entrada por tipo de contribuição |
| [Onde contribuir](https://github.com/LyannaValerie/pinker-v0/discussions/372) | Painel dinâmico de trabalho comunitário |
| [Código de Conduta](CODE_OF_CONDUCT.md) | Convivência, crítica técnica e relato de conduta |
| [Segurança](SECURITY.md) | Relato privado de vulnerabilidades |
| [Governança](GOVERNANCE.md) | Autoridade, decisões e merge manual |
| [Suporte](SUPPORT.md) | Encaminhamento para Issues e Discussions |
| `MANUAL.md` | Manual prático da linguagem implementada |
| `docs/atlas.md` | Mapa mestre da documentação |
| `docs/handoff_codex.md` | Estado operacional corrente |
| `docs/roadmap.md` | Ordem ativa oficial |
| `docs/roadmap/blocos/bloco_20.md` | Estrutura do bloco ativo |
| `docs/roadmap/bare_metal_bootstrap.md` | Convergência bare-metal e bootstrap com critérios anti-mínimo |
| `docs/history.md` | Entrada do histórico canônico |
| `docs/history/indice.md` | Índice histórico shardado |
| `docs/examples_index.md` | Índice de exemplos versionados |
| `docs/code_map.md` | Mapa rápido do código |
| `docs/expandir.md` | Critérios para expansão adulta pós-Eixo B |
| `docs/doc_rules.md` | Regras para atualização documental |
| `.github/copilot-instructions.md` | Contrato geral do GitHub Copilot no repositório |
| `.github/agents/rosa.agent.md` | Agente Rosa selecionável no GitHub Copilot |

## Desenvolvimento

| Tarefa | Comando |
|---|---|
| Preflight | `make preflight` |
| Build | `make build` |
| Testes | `make test` |
| Formatação | `make fmt-check` |
| Clippy | `make clippy` |
| Suíte oficial | `make ci` |
| Smoke | `make smoke` |

Contrato local:

- suíte oficial é stable-only;
- comandos oficiais passam por `./ci_env.sh`;
- mudança funcional exige código, testes e documentação canônica apropriada;
- documentação histórica usa `docs/history.md` e shards em `docs/history/`;
- Rosa é um agente personalizado manual; a configuração não substitui inspeção, testes ou autorização humana.

## Licenca

Veja `LICENSE`.
