# Auditoria do legado da Pinker — era dos PRs #551–600

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Décimo segundo documento da série. Cobre os **PRs #551 a #600**, mergeados entre
**30 de agosto e 5 de setembro de 2026**.

---

## 1. A era que construiu o remédio

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 13.807 | 6,83% |
| `src/` | 5.037 | 5,52% |

E no topo da concentração, um arquivo inteiro:

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| **100,0%** | **1.832 / 1.832** | **`src/intrinsics/registry.rs`** |
| 16,8% | 563 / 3.361 | `src/nav_projection_snapshot.rs` |

`src/intrinsics/registry.rs` — a autoridade declarativa única das intrínsecas
históricas — foi escrito nesta janela, pela consolidação C1 (PR #589,
`[MODULARIZAÇÃO][C1-C]`), acompanhado dos testes `c1_intrinsic_registry_tests.rs`
(406 linhas), `c2_method_dispatch_authority_tests.rs` (570) e
`c5_materialized_default_body_tests.rs` (630).

---

## 2. O que esta era prova

O terceiro documento da série (era #101–150) registrou o achado L-12: `falar`
aparece em **20 arquivos** de `src/` contra **4** de uma intrínseca governada
pelo registry.

Esta é a era em que esse registry foi construído. O diagnóstico que ele carrega
no cabeçalho é da própria equipe:

> Antes da consolidação C1 o mesmo fato existia **sete vezes**: `semantic`, `ir`,
> `ir_validate`, `cfg_ir_validate`, `instr_select_validate`,
> `abstract_machine_validate` e `backend_s` enumeravam cada um a sua cópia, e
> nada impedia que discordassem.

Sete cópias, uma autoridade, resolvido em uma janela de PRs. Sem reforma de
pipeline, sem exceção no roadmap, sem abrir fase funcional — a consolidação
entrou como trabalho de modularização.

**Este é o argumento de viabilidade mais forte que a série tem.** Toda vez que
os documentos anteriores dizem "existe remédio pronto no repositório", é deste
arquivo que estão falando. Ele custou 1.832 linhas e resolveu um problema
estruturalmente idêntico a L-07 (dois renderizadores), L-08 (dois cálculos de
layout) e L-12 (`falar` em vinte arquivos).

---

## 3. E o que ela deixou de fora

`falar` não entrou. Nem `layout.rs`. Nem os dois `render_program`.

Não digo isso como crítica — a consolidação C1 tinha escopo declarado, e cumpri-lo
sem estender é exatamente a disciplina que o projeto pratica. Digo como
localização precisa do trabalho que sobrou:

| Achado | Mesma forma de C1? | Já tem autoridade candidata? |
|---|---|---|
| L-12 (`falar` em 20 arquivos) | sim | sim — `intrinsics/registry.rs` |
| L-07 (dois `render_program`) | sim | não — precisaria nascer |
| L-08 (dois cálculos de layout) | sim | sim — `layout.rs`, basta uma derivar da outra |
| L-15 (oito `desugar_for_each`) | sim | não — precisaria nascer |

Dois dos quatro já têm onde morar. É por isso que a série recomenda L-12 e L-08
antes dos outros: não exigem inventar nada.

---

## 4. Registro acumulado

Sem acréscimo de achado. A série segue com L-01 a L-24.
