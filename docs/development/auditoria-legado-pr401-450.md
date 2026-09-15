# Auditoria do legado da Pinker — era dos PRs #401–450

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Nono documento da série. Cobre os **PRs #401 a #450**, mergeados entre **24 de
julho e 10 de agosto de 2026**.

Pré-requisitos: os oito documentos anteriores da série.

> **Aviso.** Esta era produz o achado mais consequente de toda a série, e ele
> **não é sobre código legado**. É uma divergência entre o estado documentado do
> projeto e o que a árvore contém. Está na §3.

---

## 1. A era em que o projeto dobrou

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | **73.621** | **36,40%** |
| `src/` | **31.337** | **34,35%** |

Dezessete dias produziram **mais de um terço de todo o Rust vivo do
repositório**. Nenhuma outra era chega perto.

Duas frentes correram juntas:

**Linguagem (Eixo A, Fases 242–248).** `src/ir.rs` +4.211 linhas vivas (54,2% do
arquivo), `src/interpreter.rs` +3.068 (53,9%), `runtime/pinker_rt` +3.027
(43,3%), `src/backend_s.rs` +2.046 (45,9%), `src/semantic.rs` +1.436,
`src/inline_asm.rs` 1.149 (100%). Foram entregues: valores de função e chamada
indireta, captura léxica em closures, objetos de trato e despacho dinâmico,
ponteiros crus de função, `alocar`/`liberar`, `sussurro` real e uniões
estruturais tagged.

**Infraestrutura determinística (Issue #417).** `src/nav_projection_snapshot.rs`
2.798 (83,2%), `src/diff_coverage.rs` 1.384 (100%), `src/project_state.rs` 1.068
(99,1%), `src/symbol_index.rs` 885 (97,5%), mais os testes: 5.396 linhas de
`pinker_flake_runner_tests.rs` (100%), 2.863 de `native_process_control_tests.rs`,
2.089 de `nav_projection_composition_tests.rs`, 1.850 de
`common/native_process_launcher.rs`.

---

## 2. L-22 — os testes ultrapassaram a fonte

Fato do repositório inteiro, cuja causa está nesta era:

| | linhas |
|---|---:|
| `src/` | 91.221 |
| **`tests/`** | **101.880** |

Há mais linha de teste do que de compilador. Não digo que seja errado — para um
compilador auditável, é defensável. Digo que o **objeto mais testado do
repositório não é a linguagem**:

| Artefato | Implementação | Teste dedicado | Razão |
|---|---:|---:|---:|
| `scripts/pinker-flake-runner.sh` | 1.345 linhas de shell | **5.396 linhas de Rust** | **4,0×** |
| `src/interpreter.rs` (pipeline inteira) | 5.690 | 9.957 | 1,7× |
| `src/backend_s.rs` | 4.462 | 2.869 | 0,6× |

O detector de testes instáveis — um script de shell — recebe quatro vezes mais
teste do que ele próprio tem de código, e mais teste em valor absoluto do que o
backend nativo inteiro.

Os comentários do arquivo explicam por quê, e a explicação é boa: dois falsos
verdes reais foram observados na PR #422 (um `runs` inválido que produzia
"sucesso" sem executar nada, e um código de saída truncado em módulo 256 que
transformava 256 falhas em zero). Testar isso a fundo foi a resposta certa a um
defeito que corrompia a evidência de todo o resto.

**Classificação: custo justificado, proporção não observada.** Não recomendo
reduzir esses testes. Recomendo que a proporção seja **medida**, pelo mesmo
mecanismo proposto em L-19 — se o relatório de estado disser "linguagem: X,
ferramental: Y" também para os testes, decisões como esta ficam visíveis quando
tomadas, não seis meses depois numa auditoria.

---

## 3. L-23 — o estado documentado não descreve a árvore

Este é o achado que peço que seja verificado com prioridade, porque afeta a
capacidade de saber onde o projeto está.

### 3.1 Os fatos

**Fato 1.** O commit `1177077`, de **9 de agosto de 2026**, intitulado *"feat:
remove teto artificial de call depth"*, reescreveu **464 linhas** de
`src/interpreter.rs`, removeu 130 linhas de `tests/interpreter_tests.rs` e
acrescentou `tests/d2_call_depth_tests.rs`, cujo cabeçalho diz, literalmente:

```rust
//! D2 — profundidade de chamadas independente da pilha hospedada.
```

**Fato 2.** Esse arquivo de teste contém, entre outros, o caso:

```rust
fn recursao_profunda_finita_ultrapassa_teto_historico()
```

que executa recursão profunda e exige `RuntimeValue::Int(42)`. Ele está na suíte
e passa.

**Fato 3.** Não existe teto de profundidade de chamada em `src/interpreter.rs`
hoje. A busca por `teto`, `limite`, `max_`, `depth`, `64` e `65` no arquivo
retorna apenas os tetos do domínio de uniões e da memória pública — nada de
chamadas.

**Fato 4.** `docs/handoff_codex.md`, o documento canônico de estado operacional
(precedência #3 na hierarquia de `docs/doc_rules.md`), afirma em **sete pontos**
que D2 é a próxima prioridade funcional e que sua implementação permanece
`NOT_STARTED`:

> | Trabalho ativo | **D2** — próxima prioridade funcional do Eixo A, ainda não iniciada |
>
> ```yaml
> expansao_funcional_eixo_a: D2_NEXT_FUNCTIONAL_PRIORITY
> D2:
>   status: NEXT_FUNCTIONAL_PRIORITY
>   implementation: NOT_STARTED
> ```

**Fato 5.** `docs/expandir.md`, linha 9, afirma:

> Limite executável atual: o interpretador admite 64 chamadas Pinker simultâneas
> e diagnostica a tentativa de abrir a 65ª.

Isso é contradito por um teste que passa e cujo nome diz que ultrapassa o teto
histórico.

### 3.2 A leitura

D2 foi entregue em 9 de agosto de 2026. Dois documentos canônicos — o estado
operacional e a referência de expansão — continuam descrevendo o mundo anterior a
essa entrega. `docs/expandir.md` não está desatualizado em nuance: ele afirma um
limite executável que não existe.

**Ressalva honesta.** Pode haver contexto que eu não alcancei: a entrega pode ter
sido considerada parcial, ou "D2" no teste pode nomear coisa diferente do "D2" do
roadmap. Mas o cabeçalho do teste diz `D2`, o commit diz que o teto foi removido,
e o código confirma que ele não está lá. Se a intenção era que D2 continuasse
aberto, então o que aconteceu em 9 de agosto precisa de outro nome — e o limite
de `expandir.md` precisa ser corrigido de qualquer forma.

### 3.3 Por que isto importa mais do que L-01 a L-21

Todos os achados anteriores custam esforço. Este custa **direção**. O
`handoff_codex.md` é o documento que diz a quem chega — pessoa ou agente — qual é
o próximo passo. Se ele aponta para trabalho já feito, o próximo passo é
retrabalho, e ninguém percebe até abrir o código.

**Recomendação, e ela é urgente e barata:** uma rodada documental que reconcilie
`handoff_codex.md` e `expandir.md` com a árvore. Nenhuma linha de código. É a
única recomendação desta série que eu colocaria **antes** da poda de L-01.

---

## 4. L-24 — taxonomias de trabalho sem endereço documental

Ao investigar L-23, apareceu o padrão que o explica.

A árvore contém arquivos de teste rotulados por sistemas de nomenclatura que
**não existem em `docs/`**:

| Rótulo nos testes | Arquivos | Ocorrências em `docs/` |
|---|---:|---:|
| `part_b`, `part_b1`, `part_c`, `part_d`, `part_e1`, `part_e2`, `part_g` | 9 | **0** |
| `u1_f03`, `u2_f05`, `u3_f07`, `u4_f10` | 4 | **0** |
| `d4`, `d6`, `d7`, `d8`, `d9`, `d10`, `d12`, `d13` | 8 | **0** |
| `c1`, `c2`, `c5` | 3 | 6 (só `C5`) |
| `d1`, `d2` | 2 | 19 (`D1`); D2 como não iniciado |

São **26 arquivos de teste**, somando milhares de linhas, identificados por
rótulos que a documentação canônica não conhece. E os rótulos `d4` a `d13`
sugerem uma série de treze itens da qual `docs/` registra dois.

Compare com as taxonomias que **têm** endereço documental: `Bloco` (584
ocorrências em `docs/`), `Fase` (402), `Onda` (167), `Doc-N` (120), `Faixa` (103),
`HF-N` (23), `BM-A`–`BM-D` (23), `Paralela-N` (7).

**O achado:** não é que existam muitas taxonomias — um projeto com 248 fases
precisa de vocabulário. É que **algumas delas vivem só no código**. Um arquivo
chamado `tests/d8_local_generic_inference_tests.rs` não tem como ser rastreado
até a decisão que o criou, porque essa decisão não está em lugar nenhum de
`docs/`.

E "inferência genérica local" é o **item 17 da Faixa 4** do Bloco 20. Existe um
teste dedicado a ele, de 11 de agosto de 2026, e o roadmap lista o item como não
entregue.

**Recomendação:** a mesma rodada documental de L-23 deve inventariar os 26
arquivos e dizer, para cada rótulo, o que ele é e onde ele mora. Se algum
corresponder a item de faixa do Bloco 20, isso muda o estado do roadmap.

---

## 5. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-22** | `tests/` maior que `src/`; o artefato mais testado é um script de shell | #401–450 | custo justificado, proporção não observada |
| **L-23** | D2 entregue em 09/08; `handoff_codex.md` diz `NOT_STARTED` e `expandir.md` afirma um limite que não existe | #401–450 | **divergência de estado — prioridade máxima** |
| **L-24** | 26 arquivos de teste rotulados por taxonomias ausentes de `docs/` | #401–450 | rastreabilidade rompida |
