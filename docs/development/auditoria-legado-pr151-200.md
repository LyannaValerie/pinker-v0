# Auditoria do legado da Pinker — era dos PRs #151–200

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Quarto documento da série. Cobre os **PRs #151 a #200**, mergeados entre **23 e
27 de março de 2026**.

Pré-requisitos: os três documentos anteriores da série.

---

## 1. A era que quase não sobreviveu — e por que isso é uma boa notícia

Esta é a janela com **menos código vivo** de toda a história do projeto:

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 3.133 | 1,55% |
| `src/` | **763** | **0,84%** |

763 linhas, num compilador de 91.221. Menos de um por cento.

E não foi por falta de trabalho. Cinquenta PRs em quatro dias entregaram:

| PRs | Entrega |
|---|---|
| #151–#157 | introspecção e mutação de caminho: `caminho_existe`, `e_arquivo`, `e_diretorio`, `juntar_caminho`, `tamanho_arquivo`, `criar_diretorio`, `remover_arquivo`, `escrever_verso` |
| #158 | HF-3: estabilização de handles, I/O, caminho e texto |
| #159–#172 | família texto: `contem_verso`, `comeca_com`, `termina_com`, `igual_verso`, `vazio_verso`, `aparar_verso`, `minusculo_verso`, `maiusculo_verso`, `indice_verso_em`, `anexar_verso`, `ouvir_verso` |
| #166 | Doc-18: arquitetura documental dual (Engine + Rosa), atlas e ponte |
| #174–#182 | backend externo: múltiplos blocos e labels, branch condicional, loops reais, `.rodata`, ABI até 3 args, compostos camadas 1–3, `deref_store` |
| #187–#199 | backend externo: `u32`, `u64`, `!=`, `>`, `<=`, `>=`, `quebrar`/`continuar` camadas 1–3, `ninho` heterogêneo |
| #200 | módulo `palette` |

**Para onde foi tudo isso?** Foi absorvido:

- as intrínsecas de caminho e texto foram **consolidadas** na autoridade
  declarativa de `src/intrinsics/registry.rs` pela consolidação C1 (PR #589);
- as camadas conservadoras do backend externo foram **substituídas** pela
  cobertura completa do Eixo B (Fases 212–222).

Este é o contraponto necessário a toda a série: **o projeto sabe reciclar o
próprio legado.** Quando uma dívida foi encarada de frente — por consolidação ou
por substituição — ela desapareceu, e a auditoria de hoje não a encontra. Os
achados L-01 a L-13 não descrevem um projeto incapaz de se renovar; descrevem os
pontos específicos onde a renovação ainda não passou.

---

## 2. L-14 — as 322 linhas que sobreviveram servem a uma frente pausada

O único artefato de `src/` que esta era deixou inteiro é `src/palette.rs`, do PR
#200: **312 das suas 322 linhas (96,9%) são originais**, intocadas desde 27 de
março de 2026.

O módulo define a identidade visual da Pinker em código — quinze cores nomeadas
(`KEYWORD`, `TIPO`, `STRING`, `NUMERO`, `FUNCAO`, `ERRO`, `AVISO`, `SUCESSO`,
`CURSOR`, `SELECAO`, entre outras), um `Tema`, e nove funções de formatação ANSI.

**Quem o consome:** apenas `src/editor_tui.rs`, em quatro linhas, usando duas das
nove funções (`negrito_se`, `colorir_se`) e duas constantes.

**As outras sete funções públicas — `colorir`, `colorir_com_fundo`, `negrito`,
`italico`, `sublinhado`, `sem_cor`, `resumo_paleta` — não têm um único chamador
em toda a árvore.** São sete das vinte e uma funções `pub` sem consumidor que o
primeiro documento contou; um terço do total está neste arquivo.

E `editor_tui`, o consumidor único, é a **frente pausada** registrada em
`docs/handoff_codex.md`:

> | Frente pausada | editor/TUI oficial da Pinker (Fase 136) |

### 2.1 Classificação honesta

Isto **não** é o mesmo caso de L-01. A diferença importa:

- L-01 é código morto que **fases modernas continuam editando** — custo
  recorrente e crescente;
- L-14 é código parado que **ninguém toca** — 96,9% intacto em seis meses. Custo
  recorrente: praticamente zero.

`palette.rs` não atrapalha o Bloco 20. Não tem braço de recusa, não aparece em
`match` exaustivo de instrução, não é atravessado por fase nova. Ele apenas
existe.

O que ele **é** é um caso-teste perfeito para o gate de alcançabilidade proposto
no primeiro documento. Quando a verificação entrar em `nav verificar`, `palette`
vai acusar sete funções órfãs, e a resposta certa não é apagá-las: é **declarar a
intenção**. Ou o módulo é superfície pública reservada à frente do editor — e
recebe a anotação que diz isso —, ou as sete funções sem uso saem e ficam as
duas que o editor chama.

Essa decisão custa uma linha de anotação. O valor de tomá-la é que, a partir
dela, "código parado com intenção declarada" deixa de ser indistinguível de
"código morto que ninguém notou" — que é exatamente a confusão que manteve L-01
vivo por seis meses.

### 2.2 Uma observação de identidade, não de engenharia

Vale registrar, porque a Pinker leva identidade a sério: `palette.rs` é o único
lugar do compilador onde a estética da linguagem está **codificada**, não
descrita. As cores da Rosa viram `Rgb::from_hex`. Se a frente do editor for
retomada, esse arquivo é o ponto de partida pronto; se for encerrada, a decisão
deve ser consciente, porque não é código de compilador que se apaga sem perda —
é identidade.

---

## 3. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-14** | `palette.rs`: 96,9% original, 7 de 9 funções públicas sem chamador, consumidor único é frente pausada | #151–200 | parado com intenção não declarada |

E um registro que não é achado, mas contexto: **84% do trabalho de `src/` desta
era foi absorvido ou substituído** por consolidação (C1) e por implementação real
(Eixo B). A série mede o que sobreviveu; esta era mede o que o projeto conseguiu
digerir.
