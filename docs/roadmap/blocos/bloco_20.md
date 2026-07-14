# Bloco 20 — expansão funcional rumo a SO e self-hosting (trilha por faixas)

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

## Status
Ativo. Aberto na Fase 207, junto com o encerramento do Bloco 18.

## Tese
Expandir a Pinker na direção dos dois propósitos de longo prazo do projeto:

1. **gerar um sistema operacional usando apenas Pinker**;
2. **a Pinker escrever o próprio código (self-hosting)**.

A trilha é organizada em **11 faixas ordenadas por prioridade**, mescladas a partir do inventário de lacunas frente a C, C#, C++, Python, TypeScript e Shell, mais uma **trilha transversal bare-metal e bootstrap** que fecha a passagem entre as capacidades da linguagem e um artefato freestanding verificável. A ordem entre faixas é canônica; a ordem dentro de cada faixa é recomendada, não obrigatória.

## Estrutura em dois eixos

O Bloco 20 executa em **dois eixos** que se alternam por decisão explícita:

- **Eixo A — linguagem**: as 11 faixas abaixo (superfície, tipos, funções, controle, baixo nível, metaprogramação, módulos, concorrência, I/O). Executou os itens 1–3 da Faixa 1 (Fases 208–211) e, após o Eixo B, retomou e expandiu os itens 5 → 6 → 4 → 3 → 5 → 6 nas Fases 223–240.
- **Eixo B — backend nativo**: paridade real do backend `.s` + runtime próprio com a superfície atual da linguagem (B1–B11, Fases 212–222). Aberto pela Doc-40, executado e encerrado na Fase 222.

Ordem vigente: Eixo A (itens 1–3) → **Eixo B (integral, concluído)** → Eixo A (itens 5 → 6 → 4, com lowering nativo obrigatório) → Faixa 3 → **trilha bare-metal e bootstrap** → demais faixas.

Nota de nomenclatura (Doc-41): o "B" nasceu de **B**ackend; a formalização A/B remove a ambiguidade de sequência — o Eixo A é o trilho de linguagem e veio primeiro de fato.

## Convergência estratégica

Os itens da Faixa 1 mais os três primeiros da Faixa 3 formam o conjunto que desbloqueia simultaneamente os dois propósitos: enums, pattern matching, generics, traits, error handling, closures, ponteiros de função, alocador de memória e inline assembly real. A trilha bare-metal e bootstrap transforma esse conjunto em um caminho executável sem Linux. Nenhum dos dois objetivos avança de forma sustentável sem essas duas camadas.

## Eixo A — faixas de linguagem

### Faixa 1 — funcionalidades de alta dificuldade (ex-Direção B)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 1 | Enums / tipos algébricos | Rust, TS, C# | nós de AST, estados de kernel, códigos de erro — **entregue nas Fases 208–210**: `leque` nominal com múltiplas cargas por variante (`bombom`/`verso`/leque, incl. recursão e recursão mútua); fora: carga de `ninho`/coleções e generics em leque |
| 2 | Pattern matching | Rust, C#, TS | despacho sobre enums/AST, parsing de tokens — **entregue no recorte utilizável nas Fases 209–210**: `encaixe` com despacho por variante, extração de múltiplas cargas e exaustividade no parse; fora: guards, padrões aninhados e encaixe-expressão |
| 3 | Generics mínimos (`lista<T>`, `mapa<K,V>`) | C++, TS, C# | eliminar monomorphização manual — **expandido nas Fases 211, 233, 235, 236 e 240**: `lista<T>` com T = leque + 7 intrínsecas genéricas; `mapa<K,V>` nas quatro combinações públicas `verso`/`bombom`; funções genéricas de usuário com chamada explícita `nome<T>(...)`; `leque Nome<T,...>` com monomorfização por alias concreto; fora: inferência de tipo, generics em `ninho`, bounds/especialização e coleções heterogêneas |
| 4 | Traits / interfaces mínimas | Rust, TS, C# | polimorfismo sem herança, contratos de driver — **expandido nas Fases 226–230, 232 e 234**: `trato`, `impl`, resolução nominal, receiver `ninho` opaco, cobertura completa, múltiplos contratos e desambiguação explícita; fora: objetos de trait, vtables, dynamic dispatch, coerções, default methods e overloading amplo |
| 5 | Error handling estruturado | C#, Python, Rust | recuperação sem abort, relatório de erros do compilador — **expandido nas Fases 223–224, 231, 237 e 240**: `tentar`, `propagar`, valor de sucesso nomeado, `propagar?` e base genérica declarável `Resultado<T,E>`; fora: biblioteca padrão predeclarada e integração automática com erros de runtime |
| 6 | Closures / funções anônimas | Rust, TS, Python | callbacks, iteradores, handlers — **expandido nas Fases 225, 238 e 239**: literal não capturante, função local tipada e passagem estática por especialização direta; fora: captura de ambiente, retorno de função, função armazenável ampla, ponteiro de função materializado e chamada indireta |

### Faixa 2 — consolidação do Bloco 18 (ex-Direção A) — **concluída**

Cumprida no fechamento do Bloco 18 (Fase 207): 18.6 concluído para as 7 famílias, 18.7 e 18.8 cumpridos no recorte mínimo, 18.9 e 18.10 declinados por decisão conservadora, 18.11 executado. Ver `docs/roadmap/blocos/bloco_18.md`.

### Faixa 3 — priorização natural (itens exclusivos)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 12 | Ponteiros de função / tipos função | C, C++, TS | tabelas de interrupção, callbacks, vtables |
| 13 | Alocador de memória (`alocar`/`liberar`) | C | inegociável para SO: heap próprio |
| 14 | Inline assembly real (lowering completo de `sussurro`) | C | `mov cr3`, `lgdt`, `iret` etc. |

### Trilha transversal — bare-metal e bootstrap (Doc-46)

Esta trilha não substitui nenhuma faixa e não declara suporte implementado. Ela formaliza o caminho entre o backend Linux atual e um artefato freestanding:

| Degrau | Entrega | Critério de pronto |
|---|---|---|
| BM1 | alvo freestanding x86-64 | programa sem libc, syscalls Linux ou runtime de processo hospedeiro |
| BM2 | objeto relocável | emissão `.o` com símbolos e relocations verificáveis |
| BM3 | seções e linker | `.text`, `.rodata`, `.data`, `.bss`, endereço de carga e entrada controlados |
| BM4 | entrada própria | `_start` ou equivalente configura stack, zera `.bss` e chama Pinker |
| BM5 | runtime sem host | memória, serial e abort funcionam sem `std`, libc ou Linux |
| BM6 | protocolo de boot | kernel inicializa por protocolo documentado |
| BM7 | imagem reproduzível | build produz imagem/ISO/disco com manifesto de artefatos |
| BM8 | QEMU | boot e saída serial são verificados automaticamente |
| BM9 | gate de CI | divergência de boot, saída ou retorno falha a suíte |

Detalhe e limites: `docs/roadmap/bare_metal_bootstrap.md`.

### Faixa 4 — sistema de tipos

| # | Item | Inspiração |
|---|---|---|
| 15 | Union types / tagged unions | C, C++, Rust |
| 16 | Tuplas | Python, Rust, TS |
| 17 | Inferência de tipo local | TS, Rust, C# |

### Faixa 5 — funções

| # | Item | Inspiração |
|---|---|---|
| 18 | Funções como valores (first-class) | TS, Python, C |
| 19 | Funções variádicas | C, Python |
| 20 | Parâmetros com valor padrão | Python, TS, C++ |
| 21 | Sobrecarga de funções | C++, C#, TS |

### Faixa 6 — controle de fluxo

| # | Item | Inspiração |
|---|---|---|
| 22 | `defer` / RAII / destructors | Go, Rust, C++ |
| 23 | Iteradores lazy / generators | Python, TS, C#, Rust |

### Faixa 7 — memória e baixo nível

| # | Item | Inspiração |
|---|---|---|
| 24 | Aritmética de ponteiros | C, C++ |
| 25 | Packed structs / controle de layout | C, C++ |
| 26 | Bitfields | C, C++ |
| 27 | Casting bruto ponteiro ↔ inteiro | C, C++ |
| 28 | Calling conventions / controle de ABI | C, C++ |
| 29 | Linker script / seções customizadas | C (ld) |
| 30 | Operações atômicas | C11, C++11, Rust |
| 31 | Barrier / fence de memória | C11, C++11, Rust |

### Faixa 8 — strings e metaprogramação

| # | Item | Inspiração |
|---|---|---|
| 32 | Regex mínimo | Python, TS |
| 33 | Macros / preprocessador | C, Rust |
| 34 | Reflection / introspecção | C#, Python, TS |
| 35 | Codegen / `eval` controlado | Python, Shell |

### Faixa 9 — módulos e build

| # | Item | Inspiração |
|---|---|---|
| 36 | Visibilidade (`publico`/`privado`) | Rust, TS, C# |
| 37 | Nested modules / namespaces | Rust, TS, C++ |
| 38 | Import com rename / alias | Python, TS |
| 39 | Compilação condicional | C, Rust |
| 40 | Build system completo (dependências, targets) | Cargo, npm |
| 41 | Testes integrados na linguagem | Rust, Python |

### Faixa 10 — concorrência e SO

| # | Item | Inspiração |
|---|---|---|
| 42 | Threads / coroutines | C, Rust, Go |
| 43 | Primitivas de sync (mutex, semáforo) | C, Rust, C++ |
| 44 | Async / await | TS, C#, Rust |
| 45 | IPC (pipes, shared memory, signals) | C, Shell |
| 46 | Syscall table dispatch | C |
| 47 | Interrupt handlers tipados | C, Rust |

### Faixa 11 — I/O e rede para SO

| # | Item | Inspiração |
|---|---|---|
| 48 | Raw I/O (port in/out) | C |
| 49 | Sockets / rede | C, Python, TS |
| 50 | Mmap / DMA | C |
| 51 | Block device abstraction | C (Linux) |
| 52 | Character device abstraction | C (Linux) |

## Eixo B — paridade real do backend nativo (B1–B11, Fases 212–222, encerrado)

**Origem.** Débito estrutural registrado na Doc-40: a superfície nova da linguagem precisava alcançar o backend `.s`; sem paridade nativa, o propósito "SO em Pinker" ficava estruturalmente adiado.

**Posição na trilha.** O Eixo B começou imediatamente após a Fase 211 e foi encerrado na Fase 222. Toda fase de linguagem futura entrega o lowering nativo junto.

**Decisão estratégica do eixo:**
- backend `.s` próprio x86-64 System V, sem LLVM/Cranelift e sem transpilação para C;
- runtime `pinker_rt` como staticlib Rust com ABI C estável, substituível no futuro por Pinker;
- cada fase fechou um subproblema completo com comparação interpretador × nativo.

| Fase | Prevista | Entrega | Critério de pronto |
|---|---|---|---|
| B1 | 212 | runtime `pinker_rt` + build integrado | ELF Linux real e alocador interno testado ✓ |
| B2 | 213 | ABI completa de funções | N argumentos, pilha, recursão e chamadas aninhadas ✓ |
| B3 | 214 | controle de fluxo geral | todo CFG versionado executa nativo ✓ |
| B4 | 215 | `verso` dinâmico | strings e `falar` com paridade ✓ |
| B5 | 216 | listas nativas | listas e `para cada` com paridade ✓ |
| B6 | 217 | mapas nativos | quatro mapas e iteração determinística ✓ |
| B7 | 218 | leques com carga | `encaixe` e AST recursiva nativos ✓ |
| B8 | 219 | família texto | 17 operações e compilador de brinquedo nativo ✓ |
| B9 | 220 | arquivo/caminho/tempo/acaso | paridade funcional e de sementes ✓ |
| B10 | 221 | ambiente/processo | argv real e subprocessos ✓ |
| B11 | 222 | marco de paridade | manifesto compatível nos dois modos ✓ |

**Relação com a Faixa 3.** O alocador interno do runtime não equivale à superfície `alocar`/`liberar` da linguagem.

**Relação com bare-metal.** O Eixo B prova execução nativa sobre Linux; BM1–BM9 tratam da retirada do sistema hospedeiro.

## Marcos de verificação dos propósitos

- **Marco self-hosting 1**: lexer da Pinker escrito em Pinker. Primeiro degrau verificado na Fase 209 com lexer de brinquedo.
- **Marco self-hosting 2**: parser + AST em Pinker. Fundação na Fase 210 e compilador de brinquedo na Fase 211.
- **Marco SO 0**: imagem Pinker freestanding inicia em QEMU e produz saída serial sem Linux — exige BM1–BM9 no recorte necessário.
- **Marco SO 1**: programa bare-metal com alocador próprio e handler de interrupção — exige Faixas 1, 3 e 7, trilha bare-metal e Eixo B completo.
- **Marco SO 2**: kernel mínimo com scheduler e syscalls — exige Faixas 10–11 sobre o Marco SO 1.

## Método de execução

- Cada item vira uma ou mais fases numeradas, com exemplo, testes e histórico.
- Após o Eixo B, toda fase nova entrega lowering nativo.
- Nenhum degrau BM é considerado entregue sem artefato freestanding e validação objetiva.
- A ordem entre faixas é canônica; dentro de uma faixa, itens podem ser reordenados por dependência técnica.

## Limites explícitos
- Esta trilha não reabre o Bloco 18 nem antecipa o Bloco 19.
- Nenhum item está entregue por constar aqui.
- O alvo nativo operacional atual continua sendo ELF Linux até fase funcional específica de BM.
- A trilha bare-metal não substitui memória/layout da Faixa 7 nem kernel/dispositivos das Faixas 10–11.

## Relação com os demais documentos
- `docs/roadmap.md` define a ordem ativa.
- `docs/roadmap/bare_metal_bootstrap.md` detalha BM1–BM9.
- `docs/future.md` continua sendo inventário amplo, não roadmap.
- A crônica factual vive em `docs/history/`.