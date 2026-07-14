# Pinker v0

Pinker v0 é a base factual atual da linguagem Pinker: um compilador/frontend em Rust,
com interpretador, IRs auditáveis, backend nativo próprio e documentação versionada.

Este README é a porta de entrada. Ele não tenta ser manual completo nem histórico de
fases; para isso, use os documentos apontados em [Navegação](#navegacao).

## Estado Atual

| Área | Estado |
|---|---|
| Linguagem | Frontend implementado para o recorte versionado atual |
| Runtime interpretado | `pink --run` executa a superfície estável do workspace |
| Backend nativo | `pink build --nativo` gera ELF Linux via `.s` x86-64 System V + `pinker_rt` |
| Paridade | Fases compatíveis do Eixo B verificam interpretador x nativo |
| Bloco ativo | Bloco 20: expansão rumo a SO e self-hosting |
| Fase funcional mais recente | Fase 240: leque genérico explícito para Resultado<T,E> |

## Superficie Implementada

| Frente | O que existe hoje |
|---|---|
| Base sintática | `pacote`, `trazer`, `carinho`, `mimo`, `nova`, `muda`, `talvez/senão`, laços e blocos |
| Tipos escalares | `bombom`, inteiros com largura/sinal, `logica`, `verso` |
| Dados compostos | `ninho`, arrays fixos, `leque<T...>`, `lista<T>` versionada e `mapa<K,V>` nas combinações públicas |
| Resultado | `leque` com carga, `encaixe`, `tentar`, `propagar` e `propagar?` |
| Generics | `lista<T>`, `mapa<K,V>`, `leque<T...>` via alias explícito e funções genéricas explícitas `nome<T>(...)` com monomorfização |
| Contratos | `trato`/`impl` estáticos, múltiplos contratos por tipo e desambiguação nominal |
| Funções | `carinho`, literais não capturantes, função local tipada e passagem estática como parâmetro |
| Sistema | argv, ambiente, arquivos, processos, caminhos e texto no recorte versionado |
| Ponteiros | `seta<T>`, `fragil`, deref/escrita indireta e aritmética mínima no subset atual |
| Ferramentas | CLI com check, run, IR textual, CFG, machine e build nativo |

## Limites Honestos

Pinker v0 ainda não é uma linguagem geral, nem um compilador de produção.

| Fora do recorte atual | Observação |
|---|---|
| LLVM, Cranelift, JIT e otimizações globais | O backend atual é próprio, simples e auditável |
| Multi-plataforma, múltiplas ABIs e bare-metal real | O alvo nativo atual é ELF Linux x86-64 System V |
| Runtime em Pinker | `pinker_rt` ainda vive no workspace Rust/C ABI |
| Dynamic dispatch, vtables e objetos de trait | Os contratos atuais são estáticos |
| Closures capturantes e chamada indireta ampla | Função local tipada existe como alias estático não capturante |
| Generics amplos em `ninho`/`leque` e inferência genérica | O recorte atual exige chamada genérica explícita |
| Ponteiros e layout físico completos | Há operações úteis, mas ainda conservadoras |
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

| Comando | Uso |
|---|---|
| `pink arquivo.pink` | Compila/checka o arquivo pelo caminho padrão |
| `pink --check arquivo.pink` | Validação sem execução |
| `pink --run arquivo.pink` | Execução interpretada |
| `pink --ir arquivo.pink` | Emissão de IR textual |
| `pink --cfg-ir arquivo.pink` | Emissão de CFG IR |
| `pink --machine arquivo.pink` | Emissão da machine abstrata |
| `pink build --nativo arquivo.pink` | Geração de executável nativo no recorte suportado |

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
