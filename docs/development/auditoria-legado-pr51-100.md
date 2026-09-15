# Auditoria do legado da Pinker — era dos PRs #51–100

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Segundo documento da série de auditoria de legado. Cobre a janela dos **PRs #51
a #100**, mergeados entre **18 e 20 de março de 2026**.

Pré-requisito: `docs/development/auditoria-legado-inicial.md` (era dos PRs
#1–50). Este documento **não repete** o método, as tabelas nem os achados de lá.
Ele acrescenta os achados próprios desta era, e **corrige um ponto** do anterior.

---

## 1. Registro acumulado de achados

A série passa a numerar os achados de forma contínua. Os cinco primeiros vêm do
documento anterior; os seis desta era continuam a sequência.

| ID | Achado | Era de origem | Classe | Documento |
|---|---|---|---|---|
| L-01 | Caminho morto em `backend_text` mantido por fases modernas | #1–50 | prejuízo ativo | inicial |
| L-02 | Validador textual não valida (tipos, aridade, operadores) | #1–50 | prejuízo ativo | inicial |
| L-03 | Identidade por `String` no miolo da pipeline | #1–50 | fricção | inicial |
| L-04 | `RuntimeValue` achatado: variante por instância concreta | #1–50 | fricção | inicial |
| L-05 | Diagnóstico como texto livre; span fabricado `(1,1)` | #1–50 | fricção + prejuízo ao usuário | inicial |
| **L-06** | `--asm-s` congelado no conjunto de tipos de março | #51–100 | vestigial | este |
| **L-07** | Dois renderizadores independentes do mesmo `BackendTextProgram` | #51–100 | prejuízo ativo | este |
| **L-08** | `layout.rs`: duas implementações do mesmo caminho, sem teste de concordância | #51–100 | risco latente | este |
| **L-09** | Segundo modelo de erro (`Result<_, String>`) na autoridade de layout | #51–100 | fricção | este |
| **L-10** | Explosão de monomorfização em três enums paralelos de tipo/valor | #51–100 | fricção estrutural | este |
| **L-11** | `verso`, `lista` e `mapa` sem layout estático desde março | #51–100 | limite herdado | este |

---

## 2. A era: dois dias, e a linguagem inteira

Os PRs #51 a #100 ocupam **48 horas** (18/03 04:29 → 20/03 07:04). O que entrou
nessa janela:

| PRs | Entrega |
|---|---|
| #52, #55 | operadores bitwise; `&&` e `\|\|` com short-circuit |
| #58–#61 | humanização da saída `--machine` |
| #65, #66, #68 | operador `%`; unsigned fixos `u8`–`u64`; signed fixos `i8`–`i64` |
| #69, #70, #71 | `apelido`; arrays fixos `[T; N]`; `ninho` (structs) |
| #72, #80, #82 | `seta` (ponteiro); acesso a campo e indexação; `virar` (cast) |
| #83 | `peso(tipo)` e `alinhamento(tipo)` |
| #87 | `fragil` (volatile) |
| **#88, #89, #90** | **backend textual `.s`; ABI textual mínima; assembler/linker externo** |
| #91–#94 | `sussurro` (inline asm); `livre` (freestanding); boot entry; kernel stub |
| #95, #96, #97 | `trazer` (módulos); `verso` (strings); `falar` |
| #99 | subcomando `pink build` |

Ou seja: **o sistema de tipos, o modelo de memória e o backend `.s` nasceram
inteiros em dois dias.** Toda a "Fase 42 a Fase 63" da história oficial cabe
nesta janela.

### 2.1 Sobrevivência

| Escopo | Linhas vivas hoje da era #51–100 | % do total |
|---|---:|---:|
| Repositório | 7.612 | 3,76% |
| `src/` | 4.126 | 4,52% |

Concentração (arquivos com ≥100 linhas sobreviventes desta era):

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| 74,8% | 89 / 119 | `tests/backend_s_tests.rs` |
| 66,2% | **153 / 231** | **`src/layout.rs`** |
| 52,6% | 121 / 230 | `tests/lexer_tests.rs` |
| 48,3% | 989 / 2049 | `tests/backend_s_external_toolchain_tests.rs` |
| 37,2% | 248 / 666 | `tests/abstract_machine_tests.rs` |
| 29,9% | 351 / 1173 | `tests/parser_tests.rs` |
| 28,2% | 162 / 574 | `src/pink_cli/modules.rs` |
| 26,3% | 171 / 651 | `src/printer.rs` |
| 17,8% | 234 / 1315 | `src/abstract_machine.rs` |
| 16,2% | 337 / 2075 | `src/ast.rs` |
| 11,0% | **490 / 4462** | **`src/backend_s.rs`** |
| 9,9% | 609 / 6157 | `src/semantic.rs` |
| 8,6% | 668 / 7776 | `src/ir.rs` |

Dois arquivos merecem leitura própria: `src/layout.rs`, que é dois terços de
março, e `src/backend_s.rs`, que o documento anterior descreveu de forma
incompleta.

---

## 3. Correção ao documento anterior

O documento da era #1–50 afirma:

> `backend_s.rs`, 4462 linhas, **zero** linhas da era inicial.

A frase é **verdadeira para a era #1–50 e enganosa como conclusão**. `backend_s.rs`
tem **490 linhas vivas da era #51–100**, e uma delas é estrutural: a entrada
pública `emit_from_selected`, que serve o `--asm-s`, delega o lowering a
`backend_text::lower_selected_program`.

Consequência que muda a leitura do achado L-01: **`backend_text` não é apenas a
superfície do `--pseudo-asm`.** Sua metade viva (`lower_selected_program`) é
carga estrutural do caminho `.s` textual. A poda proposta no documento anterior
continua correta e segura — ela remove apenas `lower_program`, `emit_program` e
`map_falar_args_from_cfg` — mas a frase "backend_s é moderno" precisa da
qualificação acima.

O número de linhas da era #1–50 também é refinado. O documento anterior usou
corte às 05:00 UTC de 18/03; o corte exato é o merge do PR #50, às 04:10:40 UTC.
Com o corte exato: **15.862 linhas no repositório (7,84%)** e **9.484 em `src/`
(10,40%)**, em vez de 16.134 e 9.614. A conclusão não muda.

---

## 4. As três saídas de backend, e por que isso importa

A pipeline produz assembly por **três caminhos independentes**, dois deles
nascidos nesta era:

| Superfície | Entrada | Caminho | Nascida |
|---|---|---|---|
| `--pseudo-asm` | `SelectedProgram` | `backend_text::lower_selected_program` → `backend_text::render_program` | #1–50 |
| `--asm-s` | `SelectedProgram` | `backend_text::lower_selected_program` → **`backend_s::render_program`** | #51–100 (Fases 53–55) |
| `--nativo` | `SelectedProgram` | `extract_external_callconv_program` → `render_external_x86_64_linux_callconv_impl` | Eixo B (Fase 212) |

### L-07 — dois renderizadores do mesmo modelo

Existem **duas funções públicas com a mesma assinatura**, em módulos diferentes,
serializando o mesmo tipo:

```rust
// src/backend_text.rs:772
pub fn render_program(program: &BackendTextProgram) -> String

// src/backend_s.rs:3956
pub fn render_program(program: &BackendTextProgram) -> String
```

Toda instrução nova precisa ser renderizada **duas vezes**, à mão, em dois
arquivos, com formatos de saída diferentes e sem nenhum teste que exija que
concordem sobre o que representam. O documento anterior mediu a fan-out de 12
arquivos por instrução; este é um dos motivos concretos dela.

Nem `MakeClosure`, `MakeTraitObject`, `UnionInject`, `CallIndirect` nem
`CallRaw` puderam ser acrescentadas em um lugar só.

### L-06 — `--asm-s` congelado no conjunto de tipos de março

`emit_from_selected` chama `validate_supported_subset`, que aceita apenas
`bombom`, `u8`–`i64`, `logica` e `nulo`. O próprio comentário de cartografia do
arquivo registra a divergência:

> É independente das validações incorporadas em `extract_external_callconv_program`
> (caminho montável), que aceitam um conjunto distinto de tipos (`verso`, listas,
> mapas, `seta<T>`, `ninho`).

Traduzindo: **o `--asm-s` está parado no sistema de tipos de 20 de março de
2026.** Tudo o que a Pinker ganhou desde então — `verso` dinâmico, listas, mapas,
leques, closures, tratos, uniões — é recusado por ele, enquanto o `--nativo`
executa. Mas as duas superfícies continuam anunciadas lado a lado na CLI
(`--asm` | `--asm-s` | `--s`), sem nada indicar ao usuário que uma delas é um
recorte histórico.

Há ainda uma terceira entrada, `emit_external_toolchain_subset` (variante
hospedada, sem `runtime_init`), cujos únicos consumidores são testes — o helper
`render_backend_s_external_subset` em `tests/common/mod.rs`. É superfície de
teste vivendo como API pública, o mesmo padrão que produziu L-01.

**Decisão devida — e é sua, não do gate.** `--asm-s` tem três destinos honestos:
elevá-lo ao conjunto de tipos atual (obra de porte), rebaixá-lo explicitamente a
recorte histórico documentado, ou removê-lo em favor do `--nativo`. O que não se
sustenta é oferecê-lo como se fosse par do `--nativo`.

---

## 5. `layout.rs` — 231 linhas de 19 de março que o Bloco 20 depende

`src/layout.rs` é a autoridade única de tamanho, alinhamento e offset de campo de
toda a linguagem. 153 das suas 231 linhas são desta era.

**O que está certo, e merece ser dito.** O cálculo de struct é adulto: percorre
campos, arredonda cada offset para o alinhamento do campo, acumula `max_align`, e
**arredonda o tamanho final para o alinhamento máximo** — tail padding correto.
Soma e multiplicação são `checked_add`/`checked_mul`; `round_up` verifica
overflow. Recursão de struct e de `apelido` é detectada. Para código escrito no
terceiro dia de vida do projeto, isso é bom.

### L-08 — duas implementações do mesmo caminho

O mesmo percurso de campos existe **duas vezes**, escrito de forma independente:

- `layout_of_type_inner`, braço `Type::Struct` (linhas ~115–147): calcula
  `max_align` e o tamanho final arredondado, mas **não registra os offsets**;
- `struct_field_offsets_inner` (linhas 183–220): registra os offsets, mas **não
  calcula `max_align` nem tamanho final**.

Nenhuma deriva da outra. Nenhum teste exige que concordem. Se uma mudar e a outra
não, o compilador continua compilando e o programa passa a ler o campo errado.

Isso não é hipotético para o Bloco 20. A **Faixa 7** tem os itens **25 (packed
structs / controle de layout)** e **26 (bitfields)** — os dois mudam exatamente
essa regra de arredondamento. Serão duas edições, em dois lugares, sem rede.

E as **obrigações derivadas da Fase 243** já exigem, para capturas multi-palavra:
"obter tamanho e alinhamento, alinhar o offset corrente, contabilizar padding
intermediário e avançar com soma verificada; ao final, arredondar para o
alinhamento máximo/final também com verificação de overflow". É a descrição
literal de `layout_of_type_inner`. O Bloco 20 vai precisar dessa função — e vai
encontrá-la duplicada.

**Correção conservadora, e barata:** fazer `layout_of_type_inner` derivar de
`struct_field_offsets_inner`, ou vice-versa, de modo que exista **um** percurso;
ou, se a fusão for arriscada agora, acrescentar um teste que exija concordância
entre as duas para todo struct dos exemplos versionados. O teste custa poucas
dezenas de linhas e fecha o risco imediatamente.

### L-09 — o segundo modelo de erro

`layout.rs` devolve `Result<_, String>`. Não `PinkerError`. A autoridade de
layout da linguagem **não sabe produzir um diagnóstico com span** — cada chamador
precisa embrulhar a `String` num `PinkerError` e inventar um span:

```rust
// src/union_payload.rs:216
let layout = layout::layout_of_type(ty, aliases, structs).map_err(|msg| { ... })
// src/semantic.rs:4023, 4033, 4332 — o mesmo padrão, três vezes
```

O achado L-05 do documento anterior descreveu o modelo de erro de março como
único e insuficiente. Esta era mostra que ele nem sequer é único: existe um
**segundo** modelo, `String` cru, em 13 pontos de `src/ir.rs`, 10 de
`src/tooling.rs`, 9 de `src/elf.rs`, 5 de `layout.rs`. A fronteira entre os dois
é histórica, não conceitual.

### L-11 — o que `layout.rs` recusa desde março

O braço de tipos sem layout estático é hoje uma lista de oito recusas escritas à
mão:

```rust
Type::Verso(_)            => Err("tipo 'verso' ainda não possui layout estático nesta fase")
Type::ListBombom(_)       => Err("tipo 'lista<bombom>' ...")
Type::ListVerso(_)        => Err("tipo 'lista<verso>' ...")
Type::ListEnum { .. }     => Err("tipo 'lista<Leque>' ...")
Type::MapVersoBombom(_)   => Err("tipo 'mapa<verso,bombom>' ...")
Type::MapVersoVerso(_)    => Err("tipo 'mapa<verso,verso>' ...")
Type::MapBombomBombom(_)  => Err("tipo 'mapa<bombom,bombom>' ...")
Type::MapBombomVerso(_)   => Err("tipo 'mapa<bombom,verso>' ...")
Type::Map { .. }          => Err("tipo 'mapa<K,V>' não possui layout estático; é um handle")
```

A frase "nesta fase" tem quase seis meses. `peso(verso)` e `alinhamento(verso)`
continuam impossíveis, e `verso` é tipo de primeira classe desde a Fase 61.

O ponto não é que handles devam ter layout — o comentário do último braço está
certo, handle é uma palavra e ponto. O ponto é que **a autoridade de layout não
tem uma noção de "handle"**: ela tem oito recusas nominais, uma por instância
concreta, e cada tipo novo de coleção acrescenta a nona.

---

## 6. L-10 — a explosão de monomorfização, agora com três testemunhas

O documento anterior encontrou o padrão em `RuntimeValue` e o atribuiu à era
#1–50. Estava certo quanto ao arquivo e **errado quanto à causa**. A causa é o
formato do sistema de tipos criado nesta era. Hoje o mesmo conjunto de conceitos
existe, replicado, em três enums independentes:

| Enum | Arquivo | Variantes | Papel |
|---|---|---:|---|
| `ast::Type` | `src/ast.rs` | **29** | tipo sintático |
| `ir::TypeIR` | `src/ir.rs` | **27** | tipo da IR |
| `RuntimeValue` | `src/interpreter.rs` | **15** | valor em execução |

mais os nove braços de recusa de `layout.rs`, mais `is_supported_type` de
`backend_s`, mais a classificação de representação de `union_payload`.

Acrescentar `mapa<bombom,verso>` não foi acrescentar um tipo: foi acrescentar uma
variante em `ast::Type`, uma em `TypeIR`, uma em `RuntimeValue`, um braço em
`layout.rs`, e o correspondente em cada validador e backend.

A Faixa 4 do Bloco 20 pede **tuplas** (item 16) e **inferência de tipo local**
(item 17). Tuplas, no formato atual, significam uma variante por aridade e por
combinação de tipos — ou uma reforma. Inferência local precisa de um modelo de
tipo que se possa unificar, e três enums paralelos sem relação declarada entre si
não é esse modelo.

**Não é recomendação de reforma agora.** É o registro de que a Faixa 4 vai
esbarrar nisto, e de que a decisão de encará-la deve ser tomada **antes** de
abrir a fase, não durante.

---

## 7. Ajuste à recomendação da série

A recomendação do documento anterior — acrescentar verificação de alcançabilidade
ao gate `nav verificar`, e podar o que ela autorizar — permanece **inalterada e
prioritária**. Esta era acrescenta duas coisas a ela, ambas baratas:

1. **Um teste de concordância de layout** (L-08). Poucas dezenas de linhas,
   fecha um risco silencioso, e é pré-requisito honesto para os itens 25 e 26 da
   Faixa 7. Pode entrar na mesma rodada da poda.
2. **Uma decisão declarada sobre o `--asm-s`** (L-06). Não exige código: exige
   uma frase no README e na descrição da pipeline dizendo o que aquela superfície
   é hoje. Se a decisão for elevá-la, aí sim vira fase.

Nada disso abre fase funcional, altera o roadmap ou renumera item.

---

## 8. Método

Idêntico ao do documento anterior, com o corte deslocado para a janela:
`author-time` estritamente maior que o merge do PR #50 (`2026-03-18T04:10:40Z`) e
menor ou igual ao merge do PR #100 (`2026-03-20T07:04:29Z`).

Os limites honestos declarados lá valem aqui: a atribuição é por linha, não por
decisão de projeto, e portanto **subestima** a influência da era. `layout.rs` a
66% é o piso, não o teto — o formato do arquivo inteiro é de março.
