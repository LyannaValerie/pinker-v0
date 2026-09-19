# Auditoria do legado da Pinker — era dos PRs #601–611

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Décimo terceiro documento da série, e o último por janela de PRs. Cobre os **PRs
#601 a #611**, mergeados entre **5 e 6 de setembro de 2026** — o trabalho mais
recente da `main`.

---

## 1. Onze PRs, dois dias, 9% do compilador

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 9.558 | 4,73% |
| `src/` | **8.347** | **9,15%** |

A densidade é a maior de toda a série: **87% do que esta era produziu está em
`src/`**, contra 34% da era #401–450 e 17% da era #351–400.

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| 99,3% | 3.444 / 3.468 | `src/interpreter/hosted_intrinsics.rs` |
| 100,0% | 1.282 / 1.282 | `src/parser/genericos.rs` |
| 43,3% | 1.471 / 3.401 | `src/parser/mod.rs` |
| 100,0% | 686 / 686 | `src/pink_cli/doc_cli.rs` |
| 100,0% | 658 / 658 | `src/parser/resultado.rs` |
| 100,0% | 469 / 469 | `src/parser/comandos.rs` |

---

## 2. O número engana, e é importante entender por quê

Nada disso é código novo.

Os commits da modularização física são quase simétricos:

| Commit | Inserções | Remoções |
|---|---:|---:|
| `PAR-X` — decompor `src/parser.rs` em `src/parser/` | +8.108 | −7.776 |
| `INT-1` — extrair `hosted_intrinsics.rs` do interpretador | +3.885 | −3.503 |
| `MAIN-5+2+3` — decompor `src/main.rs` em `src/pink_cli/` | +3.540 | −3.201 |
| `BS-3` — mover os testes do caminho montável | +912 | −619 |

O `git blame` com `-M -C`, usado em toda esta série, segue código movido **dentro
e entre arquivos** — mas a decomposição reescreveu cabeçalhos, `use`, assinaturas
de visibilidade e âncoras de cartografia, e essas linhas contam legitimamente
como novas.

Ou seja: **os 8.347 são reais como linhas e enganosos como trabalho.** A era
moveu 15.000 linhas de lugar e criou, em substância, muito pouco.

---

## 3. O achado: mover não é mudar de forma

Esta é a observação que mais importa para decidir o próximo passo.

A modularização física foi bem executada e resolve um problema real —
`src/parser.rs` com 8.000 linhas e `src/interpreter.rs` com 9.000 são arquivos
que ninguém navega. Depois dela, `src/parser/` tem seis arquivos com nomes que
dizem o que fazem. Isso é ganho.

Mas **nenhum dos vinte e quatro achados desta série foi tocado por ela.** Prova
direta:

- `src/parser/lacos.rs` continua com **exatamente oito** funções
  `desugar_for_each_*` (achado L-15). Elas mudaram de arquivo, não de número.
- Os dois `render_program` (L-07) continuam em `backend_text.rs` e `backend_s.rs`.
- `backend_text::lower_program` e `emit_program` (L-01) continuam `pub`, continuam
  sem chamador, e a decomposição não os viu — porque a decomposição foi física,
  e a morte deles é semântica.
- `layout.rs` (L-08) não foi tocado. `falar` (L-12) segue em vinte arquivos.

A distinção é a lição da era: **decomposição física melhora a navegação;
consolidação de autoridade melhora a estrutura.** São trabalhos diferentes, e o
projeto executou nove rodadas do primeiro tipo (`PAR-X`, `INT-1`, `MAIN-5`,
`BS-3`, `C1`–`C5`, `TRAMA/C1-B`) em duas semanas.

A consolidação C1 — a das intrínsecas, da era anterior — foi do segundo tipo. Ela
mudou a forma. As quatro desta era, não.

**Isto não é crítica ao que foi feito.** É a resposta à pergunta "o que fazer
agora": se o objetivo é reduzir o pedágio que o Bloco 20 paga por fase, mais
decomposição física não o reduz. As linhas continuam sendo as mesmas linhas, nos
mesmos vinte arquivos, só que melhor arrumadas.

---

## 4. Um ponto de atenção sobre a cartografia

Cada rodada de modularização exigiu ressincronizar o catálogo de navegação —
`src/navigation.jsonl` mudou em todas as quatro, e `tests/nav_cartography_tests.rs`
em duas. É o custo de L-21 (resumo em três cópias) cobrado a cada movimentação de
região.

Nenhuma ação recomendada além da já registrada em L-21. Registro aqui apenas
porque a era mostra a frequência real: **quatro rodadas de decomposição em dois
dias produziram quatro ressincronizações de catálogo.** Se houver mais
decomposição planejada, esse é o custo unitário a considerar.

---

## 5. Registro acumulado

Sem acréscimo de achado. A série fecha as janelas de PR com **L-01 a L-24**.

A síntese está em `docs/development/auditoria-legado-sintese.md`.
