# Auditoria do legado da Pinker — era dos PRs #351–400

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Oitavo documento da série. Cobre os **PRs #351 a #400**, mergeados entre **16 e
24 de julho de 2026**.

Pré-requisitos: os sete documentos anteriores da série.

---

## 1. A era mais desequilibrada

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 8.631 | 4,27% |
| `src/` | **1.470** | **1,61%** |

Um quinto do que a era produziu está em `src/`. O resto — **7.161 linhas — está
em testes**, e 5.331 delas num arquivo só:

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| **82,7%** | **5.331 / 6.450** | **`tests/nav_cartography_tests.rs`** |
| 40,8% | 977 / 2.394 | `src/nav.rs` |
| 56,9% | 391 / 687 | `tests/trama_query_tests.rs` |
| 49,8% | 106 / 213 | `tests/nav_catalog_tests.rs` |

O trabalho da era foi a continuação da cartografia semântica (Ondas 5E a 8) sobre
o código existente. É trabalho de ferramental — a segunda metade do L-19 — e
quase todo ele virou teste.

---

## 2. L-21 — a cartografia testa a própria prosa

`tests/nav_cartography_tests.rs` tem **6.450 linhas** e é o maior arquivo de teste
do repositório. Dentro dele há **525 asserções** e **52 literais de string com
mais de 100 caracteres**.

Esses literais não são código nem dados: são os **resumos em português** que o
catálogo de navegação guarda para cada região do código. Exemplo real, copiado do
arquivo:

```rust
"Fornece fonte inline ou exemplos versionados (fase111–125) ao helper
render_backend_s_external_subset, que executa parse, semântica, IR, CFG e
seleção em memória e emite assembly via emit_external_toolchain_subset; valida
por contains o cabeçalho do subset, `.globl main` para o entrypoint e `.local
<nome>` para toda definição não-entrypoint, rótulos locais injetivos [...]"
```

O mesmo parágrafo existe em três lugares: no comentário `@pinker-nav:summary` do
código-fonte, na linha correspondente de `src/navigation.jsonl`, e aqui, no teste.

### 2.1 A consequência

**Melhorar a redação de um resumo é editar um teste.** Corrigir um erro de
concordância, atualizar uma descrição que envelheceu, ou reescrever uma frase
confusa exige mexer em `nav_cartography_tests.rs` — e o gate `nav-check` do
`make ci` reprova se as três cópias divergirem.

Isso é a mesma patologia do achado L-05 do primeiro documento — texto de
diagnóstico virando API por causa de 2.644 `contains()` — aplicada agora à
documentação da própria cartografia. Nos dois casos, **a prosa está congelada
por teste**, e o efeito prático é o mesmo: desincentivar que ela melhore.

### 2.2 A diferença, que é a favor da Trama

Há uma justificativa real aqui que não existe no caso de L-05: a Trama tem uma
**política declarada de antirretroatividade** (Etapa 0, PR #331), e o ponto de
congelar o resumo é justamente impedir que a descrição de uma região mude sem
que alguém veja. Não é acidente; é design.

Por isso classifico L-21 como **custo consciente**, não dívida. O registro serve
para que a fundadora saiba de onde vem a sensação de peso ao mexer na
cartografia — e para nomear a única melhoria conservadora que o alivia sem
quebrar a política:

**Guardar o resumo em um lugar só.** O teste pode verificar que o resumo do
catálogo casa com o do código-fonte — que é a garantia que a política quer —
sem repetir o texto uma terceira vez dentro do próprio teste. Isso reduziria de
três cópias para duas, mantendo a antirretroatividade intacta.

Não é urgente e não bloqueia nada do Bloco 20. É a diferença entre editar um
resumo em três arquivos ou em dois.

---

## 3. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-21** | Resumos de cartografia existem em três cópias, uma delas dentro do teste | #351–400 | custo consciente |
