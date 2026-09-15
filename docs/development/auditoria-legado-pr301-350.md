# Auditoria do legado da Pinker — era dos PRs #301–350

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Sétimo documento da série. Cobre os **PRs #301 a #350**, mergeados entre **11 e
16 de julho de 2026**.

Pré-requisitos: os seis documentos anteriores da série.

---

## 1. Cinco dias, duas obras

Sobrevivência: **11.820 linhas (5,84%)**, das quais **6.927 em `src/` (7,59%)** —
a terceira maior de toda a história, atrás apenas das eras #401–450 e #451–500.

Cinquenta PRs em cinco dias fizeram **duas coisas simultâneas e independentes**:

**Obra 1 — fechar o Eixo B e retomar o Eixo A.** PRs #301–#307 entregaram B5 a
B11 (listas, mapas, leques, família texto, arquivo/caminho/tempo/acaso,
ambiente/processo, e o marco de paridade). PRs #309–#327 entregaram as Fases 223
a 240: `tentar`/`propagar`, `carinho` anônimo, `trato`, `impl`, mapas genéricos,
funções genéricas de usuário, leque genérico.

**Obra 2 — a Trama Pinker inteira.** PRs #331–#350: territórios, catálogos
documentais, navegação de código, manifestos imutáveis, projeções, CI permanente,
e as Ondas 0 a 5D de cartografia semântica.

Os arquivos que a segunda obra deixou são quase todos intocados desde então:

| % original | linhas | arquivo |
|---:|---:|---|
| **100,0%** | 458 / 458 | `src/projection.rs` |
| **100,0%** | 318 / 318 | `src/jsonl.rs` |
| **100,0%** | 144 / 144 | `src/text_norm.rs` |
| 99,8% | 1.260 / 1.263 | `src/doc_index.rs` |
| 85,9% | 535 / 623 | `src/doc.rs` |
| 72,7% | 862 / 1.186 | `src/change.rs` |
| 42,1% | 1.008 / 2.394 | `src/nav.rs` |

mais 1.608 linhas vivas em `runtime/pinker_rt/src/lib.rs`, da primeira obra.

---

## 2. L-19 — o segundo produto dentro do crate

Este é o maior achado estrutural da série, e não é sobre código velho.

O crate `pinker-v0` — cuja descrição no `Cargo.toml` é *"Pinker v0 MVP Compiler —
Minimal, Safe, Cute"* — contém hoje **dois produtos**:

| Produto | Linhas de `src/` | % de `src/` |
|---|---:|---:|
| Pipeline da linguagem (léxico → nativo, interpretador, runtime) | 71.290 | 78,2% |
| **Ferramental: Trama, cartografia, documentação, automação, tooling, editor** | **19.931** | **21,8%** |

E do lado dos testes a proporção é ainda maior:

| Produto | Linhas de `tests/` | % |
|---|---:|---:|
| Linguagem | 75.091 | 73,7% |
| **Ferramental** | **26.789** | **26,3%** |

Somando: **cerca de 46.700 das ~193.000 linhas de Rust do repositório — quase um
quarto — não são a linguagem Pinker.**

Os módulos: `nav` (2.394), `nav_projection_snapshot` (3.361),
`nav_projection_report` (702), `nav_projection_recipe` (659),
`nav_projection_lifecycle` (646), `nav_projection_store` (334), `doc` (623),
`doc_index` (1.263), `projection` (458), `jsonl` (318), `text_norm` (144),
`change` (1.186), `diff_coverage` (1.384), `project_state` (1.078),
`project_state_report` (405), `symbol_index` (908), `tooling` (941),
`automation` (2.228), `editor_tui` (311), `repl` (266), `palette` (322).

### 2.1 Isto **não** é um defeito — e a distinção é importante

Preciso ser justa, porque a leitura fácil aqui seria errada.

A Trama não é peso morto. Ela é:

- a razão de `make ci` ter `docs-check`, `nav-check` e `change-history-check`;
- a fonte de `docs/development/code-navigation-inventory.md`, que **já havia
  registrado** os achados L-01 e L-02 desta auditoria antes de mim;
- a infraestrutura sobre a qual a melhoria conservadora recomendada no primeiro
  documento — o gate de alcançabilidade — se apoia, sem precisar de ferramenta
  nova.

Sem a Trama, esta auditoria teria sido adivinhação. Com ela, os achados têm
endereço.

### 2.2 O que **é** o achado, então

O achado é que **um quarto da superfície de manutenção do repositório não é a
linguagem, e o crate não declara isso**. Consequências concretas:

- **`cargo test` mede as duas coisas juntas.** Uma quebra na cartografia e uma
  quebra no lowering de closures chegam pelo mesmo canal, com o mesmo peso.
- **O tempo de fase é compartilhado sem orçamento.** Os commits da modularização
  física dos PRs #601–611 são, em boa parte, trabalho de ferramental; nenhum item
  do Bloco 20 avançou com eles, e nenhum documento diz que não deveria.
- **A "sobriedade zero-dependency" tem custo escondido.** A decisão filosófica de
  não depender de nada é correta e eu não a contesto — mas ela significa que
  `jsonl`, `text_norm`, `projection` e o escritor JSON da AST são código próprio
  que alguém mantém. 19.931 linhas é o preço, e ele é invisível enquanto estiver
  misturado ao compilador.

### 2.3 Melhoria conservadora

**Não recomendo separar o crate.** Um workspace novo, um `pinker_trama` à parte,
seria obra grande, mexeria em `lib.rs`, em 65 módulos públicos, nos caminhos de
teste e na CI inteira — exatamente o tipo de reforma que esta série vem dizendo
para não fazer no meio do Bloco 20.

Recomendo o mínimo que torna o custo **visível**:

1. **Declarar a fronteira em `docs/code_map.md`** — uma tabela dizendo quais
   módulos de `src/` são pipeline da linguagem e quais são ferramental. Zero
   código.
2. **Relatar as duas metades separadamente** no que já existe. O `pink nav` já
   conhece territórios, domínios e camadas; a informação para somar linhas por
   território já está no catálogo. Um número no relatório de estado — "linguagem:
   X, ferramental: Y" — transforma uma proporção invisível em métrica observada.

Isso não muda uma linha de comportamento e responde à pergunta que a fundadora
fez: **onde está indo o esforço.** Hoje a resposta é "não dá para saber sem
contar à mão". Depois, dá.

---

## 3. L-20 — o que nasceu certo, e merece ser dito

`runtime/pinker_rt/src/lib.rs` começou nesta era e tem 1.608 linhas vivas dela
(23,0% do arquivo). É o runtime nativo do Eixo B.

Não é achado de dívida. É o contrário, e a série precisa registrá-lo com a mesma
firmeza com que registra os problemas: o Eixo B foi executado sob a regra "sem
recorte mínimo", entregou cobertura completa por fase, terminou com um marco de
paridade automatizado (B11) que roda o manifesto de exemplos nos dois modos
exigindo stdout e código de saída idênticos — e, seis meses depois, **não gerou
um único achado desta auditoria**.

Nenhum L-01 a L-19 aponta para `pinker_rt`, para `backend_s` pós-Eixo B, ou para
qualquer coisa entregue sob aquela regra.

Essa é a evidência mais forte de que a política do Bloco 20 está certa, e o
argumento mais direto para aplicá-la aos achados pendentes: o que foi feito
inteiro ficou inteiro.

---

## 4. Registro acumulado — acréscimo

| ID | Achado | Era | Classe |
|---|---|---|---|
| **L-19** | Um quarto do Rust do repositório é ferramental, não linguagem, sem fronteira declarada | #301–350 | custo invisível |
| **L-20** | O Eixo B, feito sob a regra "sem recorte mínimo", não gerou nenhum achado de dívida | #301–350 | evidência positiva |
