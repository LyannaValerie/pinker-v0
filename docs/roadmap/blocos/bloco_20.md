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

A trilha é organizada em **11 faixas ordenadas por prioridade**, mescladas a partir do inventário de lacunas frente a C, C#, C++, Python, TypeScript e Shell. A ordem entre faixas é canônica; a ordem dentro de cada faixa é recomendada, não obrigatória.

## Convergência estratégica

Os itens da Faixa 1 mais os três primeiros da Faixa 3 formam o conjunto que desbloqueia simultaneamente os dois propósitos: enums, pattern matching, generics, traits, error handling, closures, ponteiros de função, alocador de memória e inline assembly real. Nenhum dos dois objetivos avança de forma sustentável sem esse conjunto.

## Faixa 1 — funcionalidades de alta dificuldade (ex-Direção B)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 1 | Enums / tipos algébricos | Rust, TS, C# | nós de AST, estados de kernel, códigos de erro — **entregue nas Fases 208–210**: `leque` nominal com múltiplas cargas por variante (`bombom`/`verso`/leque, incl. recursão e recursão mútua); fora: carga de `ninho`/coleções e generics em leque |
| 2 | Pattern matching | Rust, C#, TS | despacho sobre enums/AST, parsing de tokens — **entregue no recorte utilizável nas Fases 209–210**: `encaixe` com despacho por variante, extração de múltiplas cargas e exaustividade no parse; fora: guards, padrões aninhados e encaixe-expressão |
| 3 | Generics mínimos (`lista<T>`, `mapa<K,V>`) | C++, TS, C# | eliminar monomorphização manual — **entregue no recorte utilizável na Fase 211**: `lista<T>` com T = leque + 7 intrínsecas genéricas sobre qualquer lista; fora: `mapa<K,V>` genérico, funções genéricas de usuário, generics em `leque`/`ninho` |
| 4 | Traits / interfaces mínimas | Rust, TS, C# | polimorfismo sem herança, contratos de driver |
| 5 | Error handling estruturado (`tentar/pegar` ou Result) | C#, Python, Rust | recuperação sem abort, relatório de erros do compilador |
| 6 | Closures / funções anônimas | Rust, TS, Python | callbacks, iteradores, handlers |

## Faixa 2 — consolidação do Bloco 18 (ex-Direção A) — **concluída**

Cumprida no fechamento do Bloco 18 (Fase 207): 18.6 concluído para as 7 famílias, 18.7 e 18.8 cumpridos no recorte mínimo, 18.9 e 18.10 declinados por decisão conservadora, 18.11 executado. Ver `docs/roadmap/blocos/bloco_18.md`.

## Faixa 3 — priorização natural (itens exclusivos)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 12 | Ponteiros de função / tipos função | C, C++, TS | tabelas de interrupção, callbacks, vtables |
| 13 | Alocador de memória (`alocar`/`liberar`) | C | inegociável para SO: heap próprio |
| 14 | Inline assembly real (lowering completo de `sussurro`) | C | `mov cr3`, `lgdt`, `iret` etc. |

## Faixa 4 — sistema de tipos

| # | Item | Inspiração |
|---|---|---|
| 15 | Union types / tagged unions | C, C++, Rust |
| 16 | Tuplas | Python, Rust, TS |
| 17 | Inferência de tipo local | TS, Rust, C# |

## Faixa 5 — funções

| # | Item | Inspiração |
|---|---|---|
| 18 | Funções como valores (first-class) | TS, Python, C |
| 19 | Funções variádicas | C, Python |
| 20 | Parâmetros com valor padrão | Python, TS, C++ |
| 21 | Sobrecarga de funções | C++, C#, TS |

## Faixa 6 — controle de fluxo

| # | Item | Inspiração |
|---|---|---|
| 22 | `defer` / RAII / destructors | Go, Rust, C++ |
| 23 | Iteradores lazy / generators | Python, TS, C#, Rust |

## Faixa 7 — memória e baixo nível

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

## Faixa 8 — strings e metaprogramação

| # | Item | Inspiração |
|---|---|---|
| 32 | Regex mínimo | Python, TS |
| 33 | Macros / preprocessador | C, Rust |
| 34 | Reflection / introspecção | C#, Python, TS |
| 35 | Codegen / `eval` controlado | Python, Shell |

## Faixa 9 — módulos e build

| # | Item | Inspiração |
|---|---|---|
| 36 | Visibilidade (`publico`/`privado`) | Rust, TS, C# |
| 37 | Nested modules / namespaces | Rust, TS, C++ |
| 38 | Import com rename / alias | Python, TS |
| 39 | Compilação condicional | C, Rust |
| 40 | Build system completo (dependências, targets) | Cargo, npm |
| 41 | Testes integrados na linguagem | Rust, Python |

## Faixa 10 — concorrência e SO

| # | Item | Inspiração |
|---|---|---|
| 42 | Threads / coroutines | C, Rust, Go |
| 43 | Primitivas de sync (mutex, semáforo) | C, Rust, C++ |
| 44 | Async / await | TS, C#, Rust |
| 45 | IPC (pipes, shared memory, signals) | C, Shell |
| 46 | Syscall table dispatch | C |
| 47 | Interrupt handlers tipados | C, Rust |

## Faixa 11 — I/O e rede para SO

| # | Item | Inspiração |
|---|---|---|
| 48 | Raw I/O (port in/out) | C |
| 49 | Sockets / rede | C, Python, TS |
| 50 | Mmap / DMA | C |
| 51 | Block device abstraction | C (Linux) |
| 52 | Character device abstraction | C (Linux) |

## Marcos de verificação dos propósitos

- **Marco self-hosting 1**: lexer da Pinker escrito em Pinker (exige Faixa 1 completa). **Primeiro degrau verificado na Fase 209**: lexer de brinquedo 100% em Pinker (`examples/fase209_lexer_brinquedo_valido.pink`) tokenizando fonte real com `leque` + `encaixe`.
- **Marco self-hosting 2**: parser + AST em Pinker (exige Faixas 1 e 4–5). **Fundação verificada na Fase 210** (AST recursiva avaliada em Pinker) e **verificado em miniatura na Fase 211**: compilador de brinquedo de ponta a ponta — lexer → `lista<Token>` → parser recursivo com precedência → AST → avaliação (`examples/fase211_compilador_brinquedo_valido.pink`).
- **Marco SO 1**: programa bare-metal com alocador próprio e handler de interrupção (exige Faixas 1, 3 e 7).
- **Marco SO 2**: kernel mínimo com scheduler e syscalls (exige Faixas 10–11).

## Método de execução

- Cada item vira uma ou mais fases numeradas normais, com exemplo versionado, testes e entrada no histórico.
- O padrão de suficiência conservadora continua valendo por item: recorte mínimo auditável primeiro, expansão depois.
- A ordem entre faixas é a prioridade canônica; dentro de uma faixa, itens podem ser reordenados por dependência técnica.

## Limites explícitos
- Esta trilha não reabre o Bloco 18 nem antecipa o Bloco 19 (reformas sintáticas), que segue candidato futuro.
- Nenhum item está entregue por constar aqui; entrega exige fase numerada com validação objetiva.

## Relação com os demais documentos
- `docs/future.md` continua sendo inventário amplo; esta trilha é a ordem ativa.
- A crônica factual de cada item entregue vive em `docs/history/phases/`.
