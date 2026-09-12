# Auditoria do legado da Pinker — era dos PRs #201–250

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Quinto documento da série. Cobre os **PRs #201 a #250**, mergeados entre **27 de
março e 1º de abril de 2026**.

Pré-requisitos: os quatro documentos anteriores da série.

---

## 1. A era

| Escopo | Linhas vivas hoje | % do total |
|---|---:|---:|
| Repositório | 5.454 | 2,70% |
| `src/` | 2.969 | 3,25% |

Concentração:

| % do arquivo | linhas | arquivo |
|---:|---:|---|
| **84,3%** | **1.078 / 1.279** | **`src/parser/lacos.rs`** |
| 92,6% | 288 / 311 | `src/editor_tui.rs` |
| 11,2% | 501 / 4.462 | `src/backend_s.rs` |
| 5,1% | 289 / 5.690 | `src/interpreter.rs` |
| 3,5% | 218 / 6.157 | `src/semantic.rs` |

Um arquivo domina. `src/parser/lacos.rs` — a camada de desaçucaração de laços — é
cinco sextos desta era, e é onde o achado desta janela vive.

---

## 2. L-15 — `para cada` tem oito desaçucarações escritas à mão

`lacos.rs` contém **oito funções de desaçucaração**, uma por tipo concreto de
contêiner:

```
desugar_for_each_list_enum          (linha  144)
desugar_for_each_list               (linha  266)
desugar_for_each_list_verso         (linha  383)
desugar_for_each_map                (linha  514)
desugar_for_each_map_generic        (linha  663)
desugar_for_each_map_verso_verso    (linha  811)
desugar_for_each_map_bombom_bombom  (linha  966)
desugar_for_each_map_bombom_verso   (linha 1122)
```

Cada uma ocupa entre 120 e 155 linhas, e as oito fazem a mesma coisa: criam
slots sintéticos, montam a condição, montam o corpo, montam o incremento.

O padrão é idêntico em todas — e visível no nome dos slots que cada uma fabrica:

```rust
let suffix = self.synthetic_counter;
let list_slot_name  = format!("__iter_lista_{suffix}");
let index_slot_name = format!("__iter_indice_{suffix}");
// ou, nas variantes de mapa:
let map_slot_name    = format!("__iter_mapa_{suffix}");
let size_slot_name   = format!("__iter_tamanho_{suffix}");
let cursor_slot_name = format!("__iter_cursor_{suffix}");
```

### 2.1 Por que isto é o achado L-10 em outro andar

O segundo documento registrou a explosão de monomorfização em três enums de
tipo (`ast::Type` 29 variantes, `ir::TypeIR` 27, `RuntimeValue` 15). Esta era
mostra o **mesmo fenômeno subindo para o parser**: `para cada` é **uma**
construção da linguagem, com **oito** implementações, porque não existe uma
noção de "contêiner iterável" da qual as oito derivem.

O custo de crescer é multiplicativo, não aditivo. Um `mapa<verso,logica>` novo
não acrescenta um caso: acrescenta uma nona função de ~140 linhas em `lacos.rs`,
mais a variante em `ast::Type`, mais em `TypeIR`, mais em `RuntimeValue`, mais o
braço de recusa em `layout.rs`, mais o tratamento em cada validador e backend.

E a conta é devida agora, não em tese. A **Faixa 6** do Bloco 20 pede
**iteradores lazy / generators** (item 23). Implementá-los sobre oito
desaçucarações independentes significa escrever a nona, décima e décima primeira,
ou reconhecer antes que o que falta é a abstração.

### 2.2 Por que também é o achado L-03 em ação

`lacos.rs` é o maior consumidor de identidade por `String` do compilador. Cada
uma das oito funções fabrica de dois a quatro nomes de slot por laço, e a
unicidade depende inteiramente de `synthetic_counter` — um `usize` por instância
de `Parser`, incrementado à mão em nove pontos do arquivo.

Se uma desaçucaração futura esquecer o `self.synthetic_counter += 1;` antes de
ler o sufixo, dois laços aninhados passam a compartilhar `__iter_indice_N`. O
programa compila. O laço interno sobrescreve o índice do externo.

Não afirmo que isso ocorra hoje — os nove pontos incrementam corretamente. Afirmo
que **a corretude depende de nove repetições manuais do mesmo par de linhas**, e
que nada no tipo impede a décima de errar.

### 2.3 Melhoria conservadora

Duas, em ordem de custo:

1. **Barata, agora:** extrair a alocação de sufixo para um método único
   (`fn proximo_sufixo(&mut self) -> usize`) que incrementa e devolve. Elimina as
   nove repetições e torna impossível ler um sufixo sem consumi-lo. Poucas
   dezenas de linhas, nenhuma mudança de comportamento, nenhum teste quebrado.
2. **Devida antes da Faixa 6:** um contrato único de iteração do qual as oito
   desaçucarações derivem — "o que é preciso saber para percorrer X" —, em vez de
   oito cópias do mesmo esqueleto. Isso **é** obra de porte e não deve ser feita
   dentro de uma fase de feature; deve ser a fase.

---

## 3. L-16 — a segunda superfície interativa parada

`src/editor_tui.rs` é 92,6% desta era (288 de 311 linhas) e continua **ligado ao
binário**: `src/main.rs` importa `EditorTui`, e `run_editor` abre
`EditorTui::from_path` e chama `editor.run()`.

É a frente que `docs/handoff_codex.md` lista como **pausada desde a Fase 136**.
Diferente de `palette.rs` (L-14), ela **tem** teste dedicado
(`tests/editor_tui_tests.rs`) e é alcançável pelo usuário final via CLI.

Classificação: **não é legado prejudicial.** É uma superfície de produto pausada,
testada, com consumidor real e custo de manutenção próximo de zero — não aparece
em nenhum `match` de instrução, não é atravessada por fase de linguagem.

Registro-a por uma razão só: ela e `repl.rs` (que a era seguinte trata) são as
duas superfícies interativas do `pink`, ambas paradas, ambas alcançáveis pelo
usuário, e nenhuma das duas aparece no Bloco 20. Quando o gate de alcançabilidade
entrar, elas vão acusar — e a resposta certa, como em L-14, é **declarar a
intenção**, não apagar.

---

## 4. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-15** | `para cada` com oito desaçucarações manuais; L-10 e L-03 combinados no parser | #201–250 | fricção estrutural |
| **L-16** | `editor_tui` — superfície de produto pausada, testada e ligada ao binário | #201–250 | parado com intenção não declarada |
