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

## Estrutura em dois eixos

O Bloco 20 executa em **dois eixos** que se alternam por decisão explícita:

- **Eixo A — linguagem**: as 11 faixas abaixo (superfície, tipos, funções, controle, baixo nível, metaprogramação, módulos, concorrência, I/O). Executou os itens 1–3 da Faixa 1 (Fases 208–211) e retoma nos itens 5 → 6 → 4 após o Eixo B.
- **Eixo B — backend nativo**: paridade real do backend `.s` + runtime próprio com a superfície atual da linguagem (fases B1–B11, previstas como Fases 212–222). Aberto pela Doc-40, em execução desde a Fase 212.

Ordem vigente: Eixo A (itens 1–3) → **Eixo B (integral)** → Eixo A (itens 5 → 6 → 4, agora com lowering nativo obrigatório em cada fase) → demais faixas.

Nota de nomenclatura (Doc-41): o "B" nasceu de **B**ackend; a formalização A/B remove a ambiguidade de sequência — o Eixo A é o trilho de linguagem e veio primeiro de fato.

## Convergência estratégica

Os itens da Faixa 1 mais os três primeiros da Faixa 3 formam o conjunto que desbloqueia simultaneamente os dois propósitos: enums, pattern matching, generics, traits, error handling, closures, ponteiros de função, alocador de memória e inline assembly real. Nenhum dos dois objetivos avança de forma sustentável sem esse conjunto.

## Eixo A — faixas de linguagem

### Faixa 1 — funcionalidades de alta dificuldade (ex-Direção B)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 1 | Enums / tipos algébricos | Rust, TS, C# | nós de AST, estados de kernel, códigos de erro — **entregue nas Fases 208–210**: `leque` nominal com múltiplas cargas por variante (`bombom`/`verso`/leque, incl. recursão e recursão mútua); fora: carga de `ninho`/coleções e generics em leque |
| 2 | Pattern matching | Rust, C#, TS | despacho sobre enums/AST, parsing de tokens — **entregue no recorte utilizável nas Fases 209–210**: `encaixe` com despacho por variante, extração de múltiplas cargas e exaustividade no parse; fora: guards, padrões aninhados e encaixe-expressão |
| 3 | Generics mínimos (`lista<T>`, `mapa<K,V>`) | C++, TS, C# | eliminar monomorphização manual — **entregue no recorte utilizável na Fase 211**: `lista<T>` com T = leque + 7 intrínsecas genéricas sobre qualquer lista; fora: `mapa<K,V>` genérico, funções genéricas de usuário, generics em `leque`/`ninho` |
| 4 | Traits / interfaces mínimas | Rust, TS, C# | polimorfismo sem herança, contratos de driver |
| 5 | Error handling estruturado (`tentar/pegar` ou Result) | C#, Python, Rust | recuperação sem abort, relatório de erros do compilador |
| 6 | Closures / funções anônimas | Rust, TS, Python | callbacks, iteradores, handlers |

### Faixa 2 — consolidação do Bloco 18 (ex-Direção A) — **concluída**

Cumprida no fechamento do Bloco 18 (Fase 207): 18.6 concluído para as 7 famílias, 18.7 e 18.8 cumpridos no recorte mínimo, 18.9 e 18.10 declinados por decisão conservadora, 18.11 executado. Ver `docs/roadmap/blocos/bloco_18.md`.

### Faixa 3 — priorização natural (itens exclusivos)

| # | Item | Inspiração | Motivação SO/self-hosting |
|---|---|---|---|
| 12 | Ponteiros de função / tipos função | C, C++, TS | tabelas de interrupção, callbacks, vtables |
| 13 | Alocador de memória (`alocar`/`liberar`) | C | inegociável para SO: heap próprio |
| 14 | Inline assembly real (lowering completo de `sussurro`) | C | `mov cr3`, `lgdt`, `iret` etc. |

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

## Eixo B — paridade real do backend nativo (fases planejadas B1–B11, previstas como Fases 212–222)

**Origem.** Débito estrutural registrado na Doc-40: toda a superfície nova da linguagem desde as coleções (Fases 149–211: listas, mapas, `verso` dinâmico, intrínsecas de texto/arquivo/processo, leques, `encaixe`, listas genéricas) executa **apenas no interpretador**. O backend `.s` cobre um subset linear antigo (ABI de até 3 args `bombom`, controle de fluxo parcial, `verso` estático camada 1). Cada fase de linguagem entregue sem lowering nativo **alarga** a lacuna — e sem paridade nativa, o propósito "SO em Pinker" fica estruturalmente adiado, não importa quantos itens de faixa caiam.

**Posição na trilha.** O Eixo B começa imediatamente após a Fase 211. Os itens restantes da Faixa 1 (5 — error handling, 6 — closures, 4 — traits) retomam após o eixo, com a regra permanente de que **toda fase de linguagem futura entrega o lowering nativo junto** (fim das features interpreter-only).

**Decisão estratégica do eixo** (formalizada e validada em B1):
- Caminho canônico: **backend `.s` próprio** (x86-64 System V), evoluído — não substituído.
- **Runtime nativo próprio (`pinker_rt`)**: staticlib construída no próprio workspace, com ABI C estável, linkada aos executáveis gerados. É onde vivem alocador, strings dinâmicas, coleções, leques e as intrínsecas de sistema.
- **Sem LLVM/Cranelift** (dependência pesada, contra a sobriedade zero-dependency do projeto) e **sem transpilar para C** (perderia o controle fino de ABI/memória que o propósito SO exige).
- O runtime nasce em Rust dentro do workspace (mesma toolchain já exigida, zero dependência nova) e é substituível no futuro por implementação em Pinker (convergência com a direção self-hosting).

**Regra do eixo — sem recorte mínimo.** Cada fase entrega a cobertura **completa** do seu subproblema, com critério de pronto executável e verificação contra o interpretador. Nenhuma fase fecha "no menor recorte auditável"; fecha quando o subproblema inteiro executa nativo.

| Fase | Prevista | Entrega | Critério de pronto |
|---|---|---|---|
| B1 | 212 | Runtime nativo `pinker_rt` + pipeline de build integrado — **entregue na Fase 212** | `pink build --nativo prog.pink` produz executável ELF real, montado e linkado ao runtime; alocador do runtime (`pinker_alocar`/`pinker_liberar`) funcionando sob teste; executável de fumaça roda e retorna código correto ✓ |
| B2 | 213 | ABI completa de funções — **entregue na Fase 213** | funções com N argumentos de qualquer tipo já suportado nativamente, retorno em `rax`, alinhamento de pilha e disciplina caller/callee-saved corretos; testes executáveis com 0 a 8+ argumentos e chamadas aninhadas/recursivas ✓ |
| B3 | 214 | Controle de fluxo geral — **entregue na Fase 214** | todo CFG que o pipeline produz executa nativo: `talvez`/`senao` aninhados em qualquer profundidade, `sempre que` com `quebrar`/`continuar`, cadeias completas de `escolha`/`encaixe` desugaradas, `repetir...até`, `para...de...até`, ternário via `cmov`; nenhum "bloco não suportado" restante ✓ (`encaixe` com carga aguarda o runtime de leques em B7) |
| B4 | 215 | `verso` dinâmico nativo — **entregue na Fase 215** | strings de heap no runtime (ponteiro+tamanho); literais length-prefixed, `juntar_verso`, `tamanho_verso` (Unicode), `igual_verso` e `falar` completo (`verso`/`bombom`/`logica`) executando nativo com paridade de stdout verificada ✓ (`==` de verso nativo compara ponteiro; forma canônica é `igual_verso`) |
| B5 | 216 | Listas nativas completas — **entregue na Fase 216** | `lista<bombom>`, `lista<verso>` e `lista<Leque>` com **todas** as intrínsecas (monomorphizadas e genéricas) via runtime unificado; `para cada` sobre listas nativo; paridade de stdout verificada ✓ |
| B6 | 217 | Mapas nativos completos — **entregue na Fase 217** | os 4 tipos de mapa com todas as intrínsecas + iteradores internos de `para cada` no runtime; chave `verso` compara por conteúdo; iteração nativa em ordem de inserção (determinística; o interpretador usa HashMap não determinística) ✓ |
| B7 | 218 | Leques com carga nativos — **entregue na Fase 218** | handles no runtime (`criar_0`/`anexar`/`tag`/`carga`); `encaixe` integral executando nativo, incluindo AST recursiva — o avaliador da Fase 210 nativo com paridade de stdout ✓ (o compilador da Fase 211 vira critério da B8, que lhe deve as intrínsecas de texto) |
| B8 | 219 | Família texto completa nativa — **entregue na Fase 219** | as 17 operações de texto + conversões + `formatar_verso` (wrappers por aridade, até 8 substituições) + interpolação, todas nativas com as mesmas chamadas std do interpretador; **o compilador de brinquedo da Fase 211 executa nativo com paridade de stdout** ✓ |
| B9 | 220 | arquivo + caminho + tempo + acaso nativos — **entregue na Fase 220** | ~29 funções: modelo de handles de arquivo espelhando o interpretador, caminho via std, tempo com o mesmo algoritmo civil, acaso com o **mesmo LCG** (paridade de sementes); paridade de stdout verificada em 14 linhas ✓ |
| B10 | 221 | ambiente + processo nativos | argv/env nativos (`argumento`, `ambiente_ou`, ...); `executar_processo`/`capturar_stdout`/`capturar_stderr`/`executar_com_entrada`/`pipeline_minimo` via fork/exec/pipes no runtime |
| B11 | 222 | **Marco de paridade** + fechamento do eixo | suíte automatizada executa cada exemplo versionado válido nos **dois modos** (interpretador e nativo) e exige stdout e código de saída idênticos; a paridade só é declarada com a suíte verde no CI; o compilador de brinquedo da Fase 211 executa nativo |

**Relação com a Faixa 3.** O item 13 (alocador como superfície de linguagem, `alocar`/`liberar`) é distinto e continua na Faixa 3: o Eixo B entrega o alocador **interno** do runtime; a exposição na linguagem vem depois, sobre ele.

**Relação com os marcos.** O Marco SO 1 depende diretamente do Eixo B completo (não existe programa bare-metal saindo de interpretador). O Marco self-hosting real (compilador Pinker compilando Pinker com saída executável) também: sem backend nativo com paridade, self-hosting seria interpretação sobre interpretação.

## Marcos de verificação dos propósitos

- **Marco self-hosting 1**: lexer da Pinker escrito em Pinker (exige Faixa 1 completa). **Primeiro degrau verificado na Fase 209**: lexer de brinquedo 100% em Pinker (`examples/fase209_lexer_brinquedo_valido.pink`) tokenizando fonte real com `leque` + `encaixe`.
- **Marco self-hosting 2**: parser + AST em Pinker (exige Faixas 1 e 4–5). **Fundação verificada na Fase 210** (AST recursiva avaliada em Pinker) e **verificado em miniatura na Fase 211**: compilador de brinquedo de ponta a ponta — lexer → `lista<Token>` → parser recursivo com precedência → AST → avaliação (`examples/fase211_compilador_brinquedo_valido.pink`).
- **Marco SO 1**: programa bare-metal com alocador próprio e handler de interrupção (exige Faixas 1, 3 e 7 **e o Eixo B completo**).
- **Marco SO 2**: kernel mínimo com scheduler e syscalls (exige Faixas 10–11).

## Método de execução

- Cada item vira uma ou mais fases numeradas normais, com exemplo versionado, testes e entrada no histórico.
- Nas faixas de linguagem, o alvo de cada item é o nível "utilizável pelos marcos" (Fases 208–211 são o padrão); no **Eixo B**, a regra é mais dura: cobertura completa do subproblema por fase, sem recorte mínimo.
- Após o Eixo B, toda fase de linguagem nova entrega o lowering nativo junto — features interpreter-only deixam de ser aceitas.
- A ordem entre faixas é a prioridade canônica; dentro de uma faixa, itens podem ser reordenados por dependência técnica (ordem vigente da Faixa 1: 3 → 5 → 6 → 4, com o Eixo B intercalado após o item 3).

## Limites explícitos
- Esta trilha não reabre o Bloco 18 nem antecipa o Bloco 19 (reformas sintáticas), que segue candidato futuro.
- Nenhum item está entregue por constar aqui; entrega exige fase numerada com validação objetiva.

## Relação com os demais documentos
- `docs/future.md` continua sendo inventário amplo; esta trilha é a ordem ativa.
- A crônica factual de cada item entregue vive em `docs/history/phases/`.
