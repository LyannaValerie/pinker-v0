# Auditoria do legado inicial da Pinker (era dos PRs #1–50)

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Auditoria quantitativa do código sobrevivente da primeira janela de
desenvolvimento da Pinker — os PRs #1 a #50, mergeados entre **16 e 18 de março
de 2026** — e do custo que esse código impõe às fases atuais do Bloco 20.

Este documento **não** abre fase, não altera o roadmap, não renumera itens e não
declara entrega. É referência de diagnóstico, no espírito de `docs/expandir.md`:
localizar dívida, não executá-la.

---

## 1. Método

O histórico completo da `main` (1447 commits) foi recuperado e cada linha viva
de Rust foi atribuída ao commit que a escreveu:

```
git blame --line-porcelain -w -M -C <arquivo>
```

`-w` ignora mudança de espaçamento, `-M` e `-C` seguem movimentação de código
entre e dentro de arquivos. Isso importa: sem `-M -C`, a decomposição física
recente (`src/parser/`, `src/pink_cli/`, `src/interpreter/`) apareceria
falsamente como código novo. Com elas, uma linha movida continua atribuída ao
autor original.

**Corte:** PR #50 foi mergeado em `2026-03-18T04:10:40Z`. Toda linha com
`author-time` anterior a `2026-03-18T05:00:00Z` é contada como "era dos PRs
#1–50".

**Limites honestos do método.** A atribuição é por linha, não por intenção: uma
linha reescrita em julho conta como nova mesmo que a decisão de projeto por trás
dela seja de março. O número abaixo é, portanto, um **piso** — a influência do
desenho inicial é maior que a contagem de linhas mostra. Reformatações amplas
também podem deslocar atribuição.

---

## 2. Resultado quantitativo

| Escopo | Linhas totais | Da era PR #1–50 | Proporção |
|---|---:|---:|---:|
| Rust do repositório (`src` + `runtime` + `tests` + `apps`) | 202.259 | 16.134 | **7,98%** |
| Só o compilador (`src/`) | 91.221 | 9.614 | **10,54%** |

Resposta direta à pergunta da fundadora: **sim, existe código dos primeiros 50
PRs em uso hoje — cerca de um décimo do compilador.** Três dias de trabalho de
março de 2026 ainda sustentam 9.614 linhas vivas.

### 2.1 Concentração por arquivo

Arquivos com maior densidade de linhas da era inicial (≥100 linhas antigas):

| % legado | linhas antigas / totais | arquivo |
|---:|---:|---|
| 99,4% | 491 / 494 | `tests/output_tests.rs` |
| 86,1% | 242 / 281 | `tests/backend_text_tests.rs` |
| 79,1% | 234 / 296 | `tests/backend_text_validate_tests.rs` |
| 74,7% | 701 / 939 | `tests/abstract_machine_stack_tests.rs` |
| 64,9% | 198 / 305 | `src/error.rs` |
| 64,8% | 272 / 420 | `src/backend_text_validate.rs` |
| 60,3% | 378 / 627 | `tests/cfg_ir_validate_tests.rs` |
| 49,0% | 164 / 335 | `src/token.rs` |
| 47,6% | 596 / 1251 | `src/instr_select.rs` |
| 45,8% | 750 / 1638 | `src/abstract_machine_validate.rs` |
| 44,2% | 248 / 561 | `src/lexer.rs` |
| 44,1% | 509 / 1153 | `src/backend_text.rs` |
| 42,8% | 710 / 1658 | `src/cfg_ir_validate.rs` |
| 42,3% | 877 / 2075 | `src/ast.rs` |
| 39,3% | 256 / 651 | `src/printer.rs` |
| 31,8% | 801 / 2517 | `src/cfg_ir.rs` |
| 29,0% | 381 / 1315 | `src/abstract_machine.rs` |
| 26,8% | 611 / 2277 | `src/ir_validate.rs` |
| 17,4% | 1073 / 6157 | `src/semantic.rs` |
| 13,3% | 1036 / 7776 | `src/ir.rs` |
| 6,6% | 378 / 5690 | `src/interpreter.rs` |

O padrão é nítido: **o legado não está nas pontas, está no miolo.** O frontend
(`parser/`) e o backend nativo (`backend_s.rs`, 4462 linhas, **zero** linhas
desta era) são modernos. O que é antigo é a faixa entre eles — IR, CFG,
seleção de instruções, máquina abstrata e seus validadores.

> **Errata (ver §10).** "Zero linhas desta era" vale para a era #1–50 e não
> autoriza a conclusão de que `backend_s.rs` é integralmente moderno:
> ele tem 490 linhas da era #51–100, e a entrada `--asm-s` delega o lowering
> a `backend_text`. Detalhe em `auditoria-legado-pr51-100.md`, §3.

E essa faixa **está no caminho quente dos dois produtos**:

- `--run` → `interpreter::run_program(MachineProgram)` ← `abstract_machine` ← `instr_select` ← `cfg_ir`
- `--nativo` → `backend_s::emit_from_selected(SelectedProgram)` ← `instr_select` ← `cfg_ir`

Não existe caminho de execução da Pinker que não atravesse código de março.

---

## 3. Classificação do legado

Nem todo código antigo é dívida. A divisão das 9.614 linhas de `src/`:

| Classe | Linhas | Veredito |
|---|---:|---|
| **A — fundação sadia** (`ast`, `token`, `lexer`, `printer`, `parser`) | 1.945 | Antiga e simples porque simplicidade é a resposta certa ali. **Não mexer.** |
| **B — fundação que virou imposto** (`ir`, `cfg_ir`, `instr_select`, `abstract_machine`, `semantic`, `error` e validadores) | 6.888 | Correta, mas com decisões de representação tomadas em 3 dias que hoje cobram pedágio em toda fase nova. **Fricção, não defeito.** |
| **C — vestigial / morto** (`backend_text`, `backend_text_validate`) | 781 | Prejuízo ativo. **Podar.** |

A Classe A é a boa notícia e merece ser dita: `TokenKind` nasceu com ~44
variantes e tem 94 hoje; a AST cresceu de um punhado de nós para 2.075 linhas; o
escritor JSON sem dependências de março ainda serializa a AST inteira. Essas
estruturas absorveram 200 fases sem precisar ser reescritas. Isso é um acerto de
projeto, não uma pendência.

---

## 4. Achado principal — o órgão vestigial de 16 de março

`src/backend_text.rs` contém duas funções públicas criadas em **16 de março de
2026** (commit "Adiciona camada de seleção de instruções textual", era dos PRs
#3–#7):

- `lower_program(&ProgramCfgIR)` — linhas 152–403;
- `emit_program(&ProgramCfgIR)` — linha 759;
- `map_falar_args_from_cfg` — linha 1051, chamada **apenas** de dentro de
  `lower_program`.

**Nenhuma delas tem um único chamador em toda a árvore** — nem em `src/`, nem em
`tests/`, nem em `apps/`, nem em `runtime/`. Sobrevivem porque `pinker_v0` é uma
biblioteca: item `pub` nunca dispara o lint `dead_code`.

O caminho vivo do `--pseudo-asm` é outro: a CLI usa exclusivamente
`lower_selected_program`. O próprio repositório já registrou o fato — em
`docs/development/code-navigation-inventory.md`, §7.1:

> `lower_program` (direto) e `emit_program` são `pub` **sem chamadores na
> árvore**. Paridade completa entre os dois caminhos não foi demonstrada.

marcado como **"auditoria registrada, não corrigida"**.

### 4.1 O que torna isso prejudicial e não apenas inerte

Código morto que ninguém toca custa zero. Este é tocado. Das 252 linhas de
`lower_program`, **147 foram reescritas depois de julho de 2026** — por commits
das fases mais recentes do Bloco 20:

| Data | Commit | Fase |
|---|---|---|
| 2026-07-24 | `dc5ac12` feat(language): materializa valores de função e chamada indireta | 242 |
| 2026-07-24 | `94b756e` feat(language): adiciona captura imutável em closures | 243 |
| 2026-07-26 | `a8705cf` feat(language): adiciona objetos de trato e despacho dinamico | 244 |
| 2026-07-26 | `0812c48` fix(language): corrige auditoria da Fase 244 | 244 |
| 2026-07-29 | `62c7516` feat(language): adiciona ponteiros crus de função | 245 |
| 2026-07-29 | `ae8189f` feat(runtime): emitir assembly e descritores nativos de união | 247/248 |
| 2026-07-30 | `49e0c30` feat(backend): executa encaixe tipado no interpretador e no nativo | 248 |
| 2026-08-10 | `b8037cf` Matura operandos do sussurro | — |
| 2026-08-10 | `2416757` Matura aritmética tipada de `seta<T>` | — |
| 2026-09-02 | `3e927fd` feat(identidade): separa identidade intrínseca de grafia | — |

O que essas fases escreveram ali foram **braços de recusa**: para cada instrução
nova, uma linha dizendo que o backend textual não a suporta.

```rust
InstructionCfgIR::MakeClosure { .. } => Err(PinkerError::Ir {
    msg: "backend textual ainda não lowera criação de closure (fase 243)".to_string(),
    span: crate::token::Span::single(crate::token::Position::new(1, 1)),
}),
```

Esse é o formato exato da hipótese da fundadora. **Toda fase do Bloco 20 paga
pedágio para manter compilando um caminho que ninguém executa desde o dia em que
foi escrito.** A "reciclagem de código legado" não é metáfora: é `match`
exaustivo do Rust obrigando cada feature nova a se declarar perante um órgão
morto de março.

Superfície podável: ~275 linhas em `src/backend_text.rs`, sem cobertura de teste
(nenhum teste chama as duas funções), sem mudança de comportamento observável.

### 4.2 O caminho vivo também carrega defeitos de março

O `--pseudo-asm` que **funciona** (`lower_selected_program`) tem limitações já
catalogadas em §7.2–§7.4 do inventário de navegação, todas "não corrigidas":

- os tipos de parâmetros e locais são **descartados** no modelo textual;
- o validador trata **todos** os parâmetros e locais como `TypeIR::Bombom`;
- chamadas **não têm aridade nem tipos verificados**;
- `BitNot` e `Deref` são representáveis e renderizáveis, mas o validador os
  rejeita — o modelo e o validador discordam;
- `DerefStore` e `Cast` são recusados com span sintético.

Ou seja: a superfície `--pseudo-asm` documentada no README como parte da
pipeline congelada é, hoje, o estágio **menos** verificado da cadeia — enquanto
`--nativo` e `--run` receberam a paridade completa do Eixo B.

---

## 5. Achados de fricção (Classe B)

Não são bugs. São decisões de representação de 16–17 de março que a Pinker de
2026 pagou em cada fase desde então.

### 5.1 Identidade por `String` no miolo da pipeline

Desde `feat: initial Pinker v0 frontend and IR groundwork` (16/03) e
`Adiciona alvo textual abstrato com flag --machine` (16/03):

```rust
pub enum OperandIR { Local(String), GlobalConst(String), /* ... */ Temp(TempIR) }
pub enum MachineInstr { LoadSlot(String), StoreSlot(String), LoadGlobal(String), /* ... */ }
pub struct MachineFunction { params: Vec<String>, locals: Vec<String>,
                             slot_types: HashMap<String, TypeIR>, /* ... */ }
```

Repare na assimetria: temporário é `TempIR(pub u32)` — identidade numérica — mas
slot, global e label são `String`. Duas metades do mesmo modelo com regimes de
identidade diferentes.

**Consequência prática.** Toda desaçucaração moderna precisa **fabricar nomes**:

```
src/parser/lacos.rs   __iter_lista_{n}, __iter_indice_{n}, __iter_mapa_{n},
                      __iter_cursor_{n}, __iter_tamanho_{n}, __range_limite_{n}
src/parser/mod.rs     __fnparam_{nome}_{p0_g}_{...}
closures (Fase 243)   __env, __fnref_env_{nome}
```

A unicidade não é garantida pelo tipo: depende de `synthetic_counter`, um
contador `usize` por instância de `Parser`, e de a convenção `__` ser respeitada.
O projeto já teve de criar uma autoridade única para isso —
`native_symbol::is_compiler_generated` — precisamente porque
`starts_with("__")` recusaria `__usuario`, identificador legítimo. Essa
autoridade é uma correção sobre a decisão de março, não uma escolha original.

**Custo medido.** 144 `HashMap<String, …>` em `src/`. No miolo: 102 `.clone()` em
`cfg_ir.rs`, 122 em `backend_text.rs`, 90 em `instr_select.rs` — em grande parte
clonagem de nomes que poderiam ser índices.

### 5.2 O leque de 12 arquivos por instrução nova

Uma instrução moderna precisa existir e ser tratada em toda a cadeia. Contagem
real de arquivos de `src/` que citam cada uma:

| Instrução | Fase | Arquivos de `src/` |
|---|---|---:|
| `TraitCall` | 244 | 12 |
| `CallIndirect` | 242 | 12 |
| `MakeClosure` | 243 | 11 |

E o custo por fase, medido nos commits reais:

| Commit | Arquivos `src/` tocados | Dos quais da era PR #1–50 |
|---|---:|---:|
| `dc5ac12` materializa valores de função (Fase 242) | 18 | 5 |
| `94b756e` captura imutável em closures (Fase 243) | 15 | 6 |
| `0812c48` corrige auditoria da Fase 244 | 14 | 5 |

Um terço dos arquivos que cada fase do Bloco 20 precisa editar são arquivos de
março. Esse é o "entrave de desenvolvimento" em número.

**Ressalva importante:** essa fan-out **não é toda dívida**. Ela é, em boa
medida, o preço legítimo da pipeline validada em camadas — que é um pilar de
auditabilidade da Pinker, não um acidente. O que é dívida é a parcela paga a
consumidores mortos (§4) e a validadores que não validam (§4.2).

### 5.3 `RuntimeValue` — monomorfização vazando para o modelo de valores

`enum RuntimeValue` nasceu em 17/03 (Fase 15) como `Int(u64)` + `Bool`. Hoje tem
16 variantes, das quais **nove são handles `u64` distinguidos apenas
nominalmente**:

```rust
ListBombom(u64), ListVerso(u64),
MapVersoBombom(u64), MapVersoVerso(u64), MapBombomBombom(u64), MapBombomVerso(u64),
Map(u64), SaidaProcesso(u64), ValorJson(u64), Callable(u64),
```

Cada combinação `(K,V)` de mapa virou uma variante. O modelo de valores não tem
noção de "handle de família X parametrizado por T" — tem uma variante por
instância concreta. Isso funciona hoje. Ele fica caro exatamente onde o Bloco 20
vai: Faixa 4 (tuplas, inferência de tipo local), Faixa 5 (variádicas,
sobrecarga), Faixas 10–11 (concorrência, I/O). Cada tipo genérico novo multiplica
variantes em vez de somar uma.

### 5.4 Diagnóstico como texto livre

`enum PinkerError` tem **as mesmas 11 variantes de 16 de março**, uma por estágio
da pipeline, e a mensagem é `String`. Locais de construção:

| Variante | Ocorrências |
|---|---:|
| `Semantic` | 295 |
| `Ir` | 159 |
| `Parse` | 72 |
| demais | 69 |

Praticamente todas via `format!`. Três consequências:

1. **O texto virou API pública de facto.** Há **2.644** chamadas a `contains()`
   nos testes de integração; `tests/interpreter_tests.rs` sozinho tem 275.
   Reescrever uma frase de erro quebra a suíte. Isso desincentiva melhorar
   diagnóstico — o oposto do que a Pinker quer ser.
2. **Não há identidade de erro.** Sem código estável, não há como referenciar um
   erro em documentação, agrupar por categoria, ou traduzir.
3. **Span obrigatório onde não há span.** Toda variante exige `Span`. Estágios de
   backend não têm um, e fabricam `Position::new(1, 1)` — **29 ocorrências em 13
   arquivos**. O usuário recebe um erro apontando para a linha 1, coluna 1 de um
   arquivo cujo problema está em outro lugar.

O item 3 é o único ponto desta seção que classifico como **prejuízo ao usuário
final**, não apenas fricção interna.

---

## 6. Veredito — quanto do legado é prejudicial

Das 9.614 linhas de `src/` herdadas dos PRs #1–50:

- **~1.945 linhas (20%) — sadias.** Fundação léxica e sintática. Não tocar.
- **~6.888 linhas (72%) — fricção.** Corretas e em uso. Custam pedágio por fase,
  mas trocá-las é obra de porte, não de faxina. Candidatas a expansão dirigida
  (série D), nunca dentro de uma fase de feature.
- **~781 linhas (8%) — prejuízo ativo**, das quais **~275 são poda imediata e
  segura** (o caminho morto de `backend_text`) e o restante é o validador textual
  que não valida.

Traduzindo a intuição da fundadora em veredito: **a hipótese está correta, mas o
dano é mais concentrado e menos difuso do que a suspeita sugeria.** A Pinker não
está sendo arrastada por 10% de código velho — ela está sendo arrastada por
**8% desses 10%**, num único subsistema, e a documentação do próprio projeto já
tinha o dedo no lugar certo desde a Onda 8F do inventário de navegação. O que
faltou não foi diagnóstico: foi autorização para podar.

---

## 7. Melhoria conservadora recomendada

**Um gate de alcançabilidade na Trama, e a poda que ele autoriza.**

### 7.1 Por que esta e não outra

As três candidatas naturais eram: (a) trocar identidade por `String` por índices;
(b) reformar `PinkerError` com códigos estáveis; (c) podar o caminho morto.

(a) e (b) são reformas de porte que tocam milhares de linhas e 2.644 asserções de
teste — exatamente o tipo de obra que `docs/expandir.md` manda planejar como
expansão dirigida, não improvisar. Elas ficam registradas aqui como candidatas à
série D, **não recomendadas agora**.

A recomendação é (c), com um acréscimo que a transforma de faxina em mecanismo.

### 7.2 O que fazer

**Passo 1 — o mecanismo (futuro).** A Pinker já tem toda a maquinaria necessária:
`src/nav.rs`, `src/symbol_index.rs`, o catálogo da Trama e o alvo `nav-check`
(`pink nav verificar`), que **já roda em `make ci`**. A proposta é acrescentar ao
`nav verificar` uma verificação de alcançabilidade:

> Toda região cartografada que declara um símbolo `pub` deve ter ao menos um
> consumidor na árvore, **ou** declarar-se explicitamente como superfície
> pública intencional (uma anotação `@pinker-nav:` própria).

Nada de dependência nova, nada de ferramenta nova, nada de fase nova — é um campo
a mais num gate que já existe e já é obrigatório.

**Passo 2 — a poda (presente).** Com o gate no lugar, remover:

- `backend_text::lower_program` (linhas 152–403);
- `backend_text::emit_program` (linha 759);
- `map_falar_args_from_cfg` (linha 1051), órfã depois das duas.

~275 linhas, zero chamadores, zero cobertura de teste, zero mudança de
comportamento. Fecha o item §7.1 do inventário de navegação, que está aberto
desde a Onda 8F.

**Passo 3 — decidir sobre o `--pseudo-asm` vivo.** A poda não resolve §4.2. Aqui
o gate não decide; a fundadora decide. Duas saídas honestas, e nenhuma terceira:

- **elevar**: o validador textual passa a carregar tipos, aridade e os operadores
  que o modelo já representa — vira uma expansão de série D com critério de
  pronto claro; ou
- **rebaixar**: `--pseudo-asm` é declarado superfície de *inspeção didática*, não
  estágio validado, e o README e a descrição da pipeline congelada dizem isso com
  todas as letras.

O que não se sustenta é o estado atual: um estágio anunciado como validado cujo
validador trata todo parâmetro como `bombom` e não confere aridade.

### 7.3 Por que serve à Pinker futura

O Bloco 20 tem uma regra permanente — *"toda fase de linguagem nova entrega o
lowering nativo junto; features interpreter-only deixam de ser aceitas"*. Hoje
essa regra é sustentada por **disciplina humana**. O gate de alcançabilidade a
transforma em algo que o CI sabe verificar: um estágio que ninguém consome não
passa despercebido, e um braço de recusa acrescentado a um consumidor morto vira
falha de CI em vez de linha de diff aceita sem comentário.

As faixas que faltam no Bloco 20 são as mais largas do roadmap — Faixa 7 (oito
itens de baixo nível), Faixas 10–11 (concorrência, SO, rede), mais as quatro
frentes BM-A a BM-D. Cada uma vai acrescentar instruções, estágios e superfícies.
O momento de instalar o mecanismo que impede órgãos vestigiais é **antes** dessa
expansão, não depois — foi exatamente o que não aconteceu em março de 2026, e a
conta chegou 200 fases depois.

### 7.4 Aderência à identidade

- **Minimal**: a entrega remove código e acrescenta um campo a um gate existente.
  Não cria ferramenta, documento estrutural nem dependência.
- **Safe**: fortalece o pilar de auditabilidade — passa a existir prova mecânica
  de que a pipeline não carrega estágios órfãos.
- **Cute**: não toca vocabulário, sintaxe, estética nem superfície de linguagem.
  Nenhum programa `.pink` existente muda de comportamento.

### 7.5 Riscos

| Risco | Avaliação |
|---|---|
| `emit_program`/`lower_program` serem API pública intencional para terceiros | Baixo. O inventário de navegação as classifica como achado de auditoria, não como superfície declarada. **Confirmação da fundadora é necessária antes da poda.** |
| O gate acusar falso positivo em símbolos usados só por testes de integração | Real e esperado: testes em `tests/` são consumidores externos da lib. O gate precisa contar `tests/` como consumidor legítimo — e a anotação de superfície intencional cobre o resto. |
| Ruído inicial | Hoje há **21** `pub fn` em `src/` sem chamador fora do próprio arquivo (20 modernos, 1 da era inicial). São poucos: o gate nasce com backlog pequeno e administrável. |
| Governança documental | A poda é mudança estrutural de código e exige, por `docs/doc_rules.md`, registro no território afetado e no handoff. Não é dispensável. |

---

## 8. O que este documento não recomenda

- **Não** reescrever `OperandIR`/`MachineInstr` para identidade numérica agora.
- **Não** reformar `PinkerError` agora.
- **Não** achatar `RuntimeValue` agora.
- **Não** tocar `ast.rs`, `token.rs`, `lexer.rs`, `printer.rs` — são o legado que
  deu certo.
- **Não** abrir fase funcional para nada disto: a poda é obra de rodada, e os
  itens de §5 são candidatos à série D, sujeitos a decisão humana separada.

---

## 9. Reprodução

```bash
git fetch --unshallow origin main

# atribuição por linha, seguindo movimentação de código
for f in $(git ls-files 'src/*.rs' 'runtime/*.rs' 'tests/*.rs'); do
  git blame --line-porcelain -w -M -C "$f" \
    | awk -v F="$f" '/^author-time /{t=$2} /^\t/{print F"\t"t}'
done > blame_raw.txt

# corte: PR #50 mergeado em 2026-03-18T04:10:40Z
CUT=$(date -u -d '2026-03-18T05:00:00Z' +%s)
awk -v C=$CUT '{tot++; if($2<=C) old++} END{printf "%d/%d (%.2f%%)\n", old, tot, 100*old/tot}' blame_raw.txt

# manutenção sobre o caminho morto
git log origin/main --format='%ad %h %s' --date=short -L 152,403:src/backend_text.rs
```


---

## 10. Errata e continuidade da série

Este documento abriu uma série. Correções e refinamentos vindos das eras
seguintes ficam registrados aqui, sem reescrever a análise original.

- **Corte exato.** As contagens acima usam corte às 05:00 UTC de 18/03/2026. O
  merge do PR #50 foi às 04:10:40 UTC. Com o corte exato, os números são
  **15.862 linhas no repositório (7,84%)** e **9.484 em `src/` (10,40%)**, em vez
  de 16.134 e 9.614. A classificação da §3 e o veredito da §6 não mudam.
- **`backend_s.rs`.** Ver a errata na §2.1 e
  `docs/development/auditoria-legado-pr51-100.md`, §3.
- **Causa da explosão de `RuntimeValue` (§5.3).** O arquivo está certo; a
  atribuição da causa à era #1–50 está incompleta. O formato replicado em três
  enums de tipo nasce na era #51–100 — ver aquele documento, §6 (achado L-10).
- **Numeração de achados.** A partir do segundo documento, a série mantém um
  registro contínuo. Os achados deste documento são L-01 a L-05.

Documentos da série:

A série cobre os PRs #1 a #611 em treze janelas, mais a síntese. O índice
completo e o veredito consolidado estão em
`docs/development/auditoria-legado-sintese.md`.
