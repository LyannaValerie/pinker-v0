# Auditoria do legado da Pinker — era dos PRs #251–300

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Sexto documento da série. Cobre os **PRs #251 a #300**, mergeados entre **1º de
abril e 11 de julho de 2026**.

Pré-requisitos: os cinco documentos anteriores da série.

---

## 1. A era do hiato

Esta janela é a mais longa em tempo de calendário e a mais descontínua da
história do projeto. Cinquenta PRs cobrem **três meses e dez dias**, mas o
trabalho não está distribuído neles:

| Trecho | Período | Ritmo |
|---|---|---|
| #251–#290 | 1 a 12 de abril | ~40 PRs em 11 dias |
| #291 | **21 de maio** | 1 PR |
| **hiato** | **21/05 → 09/07** | **sete semanas sem merge** |
| #292–#300 | 9 a 11 de julho | 9 PRs em 3 dias |

Sobrevivência: **5.557 linhas (2,75%)**, das quais **2.680 em `src/` (2,94%)**.

Concentração: `semantic.rs` (642), `ir.rs` (351), `runtime/pinker_rt` (303 — as
primeiras linhas vivas do runtime nativo), `interpreter.rs` (263),
**`src/repl.rs` (170, 63,9% do arquivo)**, `backend_s.rs` (166),
**`src/lexer.rs` (150, 26,7%)**.

O que entrou antes do hiato: controle de processo (Fases 162–170 — captura de
stdout/stderr, stdin textual, pipe entre processos, argv explícito), o REPL
(Fase 167), a norma visual (Fase 171), a convenção de imports (Fases 174–175) e
a abertura do Bloco 18 com as famílias temáticas (Fases 180–189).

O que entrou depois: a retomada — Bloco 18 encerrado, **Bloco 20 aberto**, e o
Eixo B começando (B2, B3, B4).

---

## 2. L-17 — a terceira superfície interativa

`src/repl.rs` nasceu na Fase 167 (PR #258, "repl mínimo auditável") e é **63,9%
original** (170 de 266 linhas).

Com ele, o `pink` tem **três** superfícies interativas, todas paradas:

| Superfície | Arquivo | Nasceu | Estado |
|---|---|---|---|
| Editor TUI | `src/editor_tui.rs` | Fase 136 | pausada (registrada no handoff) |
| REPL | `src/repl.rs` | Fase 167 | sem menção no Bloco 20 |
| Paleta | `src/palette.rs` | PR #200 | serve só o editor (L-14) |

E `repl.rs` repete o padrão de L-14: **`run_repl_with_io` é `pub` e não tem um
único chamador na árvore** — é uma das vinte e uma funções órfãs contadas no
primeiro documento. A variante usada, `run_repl()`, é chamada de `main.rs`; a
variante injetável de I/O, escrita para permitir teste, ficou sem consumidor.

Classificação: mesma de L-14 e L-16 — **parado com intenção não declarada**, não
prejuízo ativo. Nenhuma das três aparece em `match` de instrução, nenhuma é
atravessada por fase de linguagem, nenhuma cobra pedágio do Bloco 20.

O registro serve a um propósito só: quando o gate de alcançabilidade entrar, as
três vão acusar juntas, e convém que a fundadora saiba de antemão que são três
frentes de produto — não três esquecimentos.

---

## 3. L-18 — o fechamento "por suficiência conservadora"

Três PRs desta era carregam a mesma fórmula no título:

- #269 — "fecha bloco 17 por suficiência conservadora"
- #271 — "fase 178: fecha 16.2 por suficiência conservadora"
- #272 — "docs: fechar Bloco 16 por suficiência conservadora (Fase 179)"

E o fechamento do Bloco 18, já depois do hiato, registra em `bloco_20.md`:

> 18.7 e 18.8 cumpridos no **recorte mínimo**, 18.9 e 18.10 **declinados por
> decisão conservadora**.

Isto não é código legado e não aparece em nenhuma contagem de linhas. É a
**forma de encerrar** que a era praticou: um bloco fecha não quando o trabalho
acaba, mas quando o que existe é declarado suficiente.

### 3.1 Por que isto pertence a uma auditoria de legado

Porque é a origem documental do problema que `docs/expandir.md` foi criado para
catalogar. Aquele documento mede, sobre os títulos das fases históricas:

> | Títulos de fase inspecionados | 214 |
> | Títulos marcados por padrão mínimo/conservador/recorte/pequeno | 113 |
> | Proporção aproximada | **52,8%** |

Mais da metade dos marcos funcionais nomeados carregam a marca de entrega mínima.
A auditoria de código desta série mede a consequência disso do lado do código; a
Doc-42 mediu a causa do lado do processo. **São o mesmo fenômeno visto de dois
ângulos**, e vale que os dois documentos se citem.

### 3.2 O que mudou, e isso é a boa notícia

O Bloco 20 **reverteu a política explicitamente**, e a reversão está escrita:

> **Regra do eixo — sem recorte mínimo.** Cada fase entrega a cobertura
> **completa** do seu subproblema. [...] Nenhuma fase fecha "no menor recorte
> auditável".

e, para o Eixo A depois do Eixo B:

> o alvo de cada item deixa de ser o menor recorte auditável por hábito [...] o
> padrão passa a ser implementação adulta.

A era #251–300 é, portanto, o **último território** onde a forma antiga operou
sem contestação. O que ela deixou não é código ruim — é um conjunto de blocos
fechados cujo fechamento significa "decidimos parar aqui", não "acabou".

**Recomendação:** nenhuma ação de código. Uma ação documental barata e útil:
que `docs/expandir.md` liste nominalmente os fechamentos por suficiência
conservadora (Blocos 16, 17, item 16.2, itens 18.7–18.10) como entradas do seu
inventário, para que a dívida tenha nome e endereço em vez de existir só como
percentual de títulos.

---

## 4. Uma nota sobre o hiato

Sete semanas sem merge, entre 21 de maio e 9 de julho, separam a era em duas. O
que voltou depois não foi continuidade: foi o encerramento do Bloco 18 e a
abertura do Bloco 20 com uma política declaradamente diferente.

Registro isto porque uma auditoria de legado que só conta linhas perderia o fato
mais explicativo desta janela: **a mudança de padrão do projeto não foi gradual,
foi uma retomada.** Os achados L-01 a L-18 estão, quase todos, do lado de cá da
linha — escritos antes de 21 de maio. Não é coincidência, e não é crítica: é a
razão de a poda proposta no primeiro documento ser barata. O que está velho está
velho de um jeito específico, datado e delimitado.

---

## 5. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-17** | `repl.rs` — terceira superfície interativa parada; `run_repl_with_io` sem chamador | #251–300 | parado com intenção não declarada |
| **L-18** | Fechamento "por suficiência conservadora" de Blocos 16, 17 e itens 18.7–18.10 | #251–300 | dívida de processo, já revertida no Bloco 20 |
