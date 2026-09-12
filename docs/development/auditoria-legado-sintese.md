# Auditoria do legado da Pinker — síntese da série

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Fechamento da auditoria de legado por janelas de 50 PRs, dos **PRs #1 a #611**.
Consolida os treze documentos por era num veredito único e numa ordem de ação.

Este documento **não** abre fase, não altera o roadmap, não renumera item e não
executa nenhuma das recomendações.

---

## 1. Como o repositório se distribui pela própria história

Cada linha viva de Rust foi atribuída ao commit que a escreveu
(`git blame -w -M -C` sobre os 1.447 commits da `main`) e agrupada pela janela de
50 PRs em que nasceu.

| Janela | Período | Linhas vivas | % repo | `src/` | % `src/` |
|---|---|---:|---:|---:|---:|
| #1–50 | 16–18 mar | 15.862 | 7,84% | 9.484 | 10,40% |
| #51–100 | 18–20 mar | 7.612 | 3,76% | 4.126 | 4,52% |
| #101–150 | 20–23 mar | 4.472 | 2,21% | 2.568 | 2,82% |
| #151–200 | 23–27 mar | 3.133 | 1,55% | 763 | 0,84% |
| #201–250 | 27 mar–1 abr | 5.454 | 2,70% | 2.969 | 3,25% |
| #251–300 | 1 abr–11 jul | 5.557 | 2,75% | 2.680 | 2,94% |
| #301–350 | 11–16 jul | 11.820 | 5,84% | 6.927 | 7,59% |
| #351–400 | 16–24 jul | 8.631 | 4,27% | 1.470 | 1,61% |
| **#401–450** | **24 jul–10 ago** | **73.621** | **36,40%** | **31.337** | **34,35%** |
| #451–500 | 10–22 ago | 30.427 | 15,04% | 10.816 | 11,86% |
| #501–550 | 22–30 ago | 12.305 | 6,08% | 4.697 | 5,15% |
| #551–600 | 30 ago–5 set | 13.807 | 6,83% | 5.037 | 5,52% |
| #601–611 | 5–6 set | 9.558 | 4,73% | 8.347 | 9,15% |
| **Total** | | **202.259** | | **91.221** | |

Três leituras que só aparecem com a tabela inteira à vista:

1. **As cinco primeiras janelas — os primeiros 250 PRs, treze dias de março —
   deixaram 19.910 linhas vivas em `src/`: 21,8% do compilador de hoje.**
2. **Metade do repositório atual nasceu em cinco semanas** (#401–500,
   24 jul–22 ago): 104.048 linhas, 51,4% do total.
3. A janela mais recente (#601–611) parece densa — 9,15% de `src/` em dois dias —
   mas é modularização física: os commits são quase simétricos
   (+8.108/−7.776 em `PAR-X`). Moveu, não criou.

---

## 2. O registro completo de achados

Vinte e quatro achados, classificados por natureza e não por gravidade
percebida.

### 2.1 Prejuízo ativo — custa a cada fase nova

| ID | Achado | Era |
|---|---|---|
| **L-01** | `backend_text::lower_program`/`emit_program`: sem chamador desde 16/03, e ainda assim 147 das 252 linhas reescritas por commits das Fases 242–248 | #1–50 |
| **L-02** | O validador textual do `--pseudo-asm` trata todo parâmetro como `bombom`, não confere aridade e rejeita operadores que o modelo representa | #1–50 |
| **L-07** | Duas funções `pub fn render_program(&BackendTextProgram)`, em módulos diferentes, mantidas à mão em paralelo | #51–100 |

### 2.2 Divergência de estado — custa direção

| ID | Achado | Era |
|---|---|---|
| **L-23** | D2 entregue em 09/08/2026; `handoff_codex.md` afirma `NOT_STARTED` em sete pontos e `expandir.md` afirma um limite de 64 chamadas que o código não tem | #401–450 |
| **L-24** | 26 arquivos de teste rotulados `part_*`, `u*_f*`, `d4`–`d13` — zero ocorrências em `docs/`; entre eles um teste de inferência genérica local, item 17 da Faixa 4 listado como não entregue | #401–450 |

### 2.3 Fricção estrutural — não quebra nada, encarece tudo

| ID | Achado | Era |
|---|---|---|
| **L-03** | Identidade por `String` no miolo (`LoadSlot(String)`, `slot_types: HashMap<String, TypeIR>`), obrigando fabricação de nomes sintéticos | #1–50 |
| **L-04** | `RuntimeValue` com variante por instância concreta; nove das quinze são handles `u64` distinguidos só nominalmente | #1–50 |
| **L-05** | Diagnóstico como texto livre: 11 variantes desde março, 2.644 `contains()` nos testes, 29 spans fabricados `(1,1)` | #1–50 |
| **L-09** | Segundo modelo de erro (`Result<_, String>`) na autoridade de layout, sem span | #51–100 |
| **L-10** | Mesmos conceitos de tipo replicados em `ast::Type` (29), `ir::TypeIR` (27) e `RuntimeValue` (15), mais os braços de `layout.rs` | #51–100 |
| **L-12** | `falar` fora do registry: 20 arquivos contra 4 de uma intrínseca governada | #101–150 |
| **L-15** | `para cada` com oito desaçucarações manuais de 120–155 linhas cada | #201–250 |

### 2.4 Risco latente — funciona hoje, sem rede

| ID | Achado | Era |
|---|---|---|
| **L-08** | `layout.rs` calcula o percurso de campos duas vezes, de forma independente, sem teste de concordância | #51–100 |
| **L-13** | Nove contadores de handle independentes começando em 1; a única defesa é a variante nominal | #101–150 |

### 2.5 Limites herdados e superfícies paradas

| ID | Achado | Era |
|---|---|---|
| **L-06** | `--asm-s` congelado no conjunto de tipos de 20/03, oferecido na CLI como par do `--nativo` | #51–100 |
| **L-11** | `verso`, `lista` e `mapa` sem layout estático; `peso(verso)` impossível desde março | #51–100 |
| **L-14** | `palette.rs`: 96,9% original, 7 de 9 funções públicas sem chamador | #151–200 |
| **L-16** | `editor_tui` — frente pausada, testada, ligada ao binário | #201–250 |
| **L-17** | `repl.rs` — terceira superfície interativa parada; `run_repl_with_io` sem chamador | #251–300 |
| **L-18** | Fechamento "por suficiência conservadora" de Blocos 16, 17 e itens 18.7–18.10 | #251–300 |

### 2.6 Custos conscientes e evidências positivas

| ID | Achado | Era |
|---|---|---|
| **L-19** | Um quarto do Rust do repositório é ferramental, não linguagem, sem fronteira declarada | #301–350 |
| **L-21** | Resumos de cartografia em três cópias, uma dentro do teste | #351–400 |
| **L-22** | `tests/` (101.880) maior que `src/` (91.221); o artefato mais testado é um script de shell | #401–450 |
| **L-20** | O Eixo B, feito sob a regra "sem recorte mínimo", não gerou nenhum achado de dívida | #301–350 |

---

## 3. O veredito

A hipótese que abriu esta auditoria era que o código antigo estivesse
prejudicando a Pinker. A resposta é **sim, mas não do jeito que a intuição
sugeria**, e a diferença muda o que fazer.

**O que a intuição sugeria:** que 10% de código velho no compilador estivesse
arrastando o desenvolvimento de forma difusa.

**O que a medição mostra:**

- **A fundação de março é majoritariamente sadia.** `ast.rs`, `token.rs`,
  `lexer.rs` e `printer.rs` absorveram 248 fases sem reescrita. `layout.rs`
  calcula tail padding corretamente com aritmética verificada. Isso é acerto de
  projeto, e apagá-lo em nome de "modernizar" seria perda.
- **O prejuízo ativo é pequeno e localizado.** Três achados (L-01, L-02, L-07),
  concentrados num subsistema — o backend textual — somando algumas centenas de
  linhas. É aí que o pedágio por fase é cobrado.
- **A fricção é real e é de forma, não de idade.** L-03, L-04, L-10, L-12 e L-15
  não são "código velho": são **uma decisão de representação tomada em três dias
  de março** que nunca teve uma fase própria para ser revista. Ela envelheceu bem
  em corretude e mal em custo marginal.
- **O achado mais caro não é código.** L-23 e L-24 são divergências entre o
  estado documentado e a árvore. Elas não custam esforço; custam **direção** — o
  documento que diz qual é o próximo passo aponta para trabalho já feito.
- **E o projeto sabe se curar.** A era #151–200 teve 84% do seu trabalho de
  `src/` absorvido por consolidação ou substituído por implementação real. O Eixo
  B não gerou um único achado. A consolidação C1 resolveu, em uma janela de PRs,
  um problema estruturalmente idêntico a três dos que continuam abertos.

**Resposta curta para a fundadora:** o código antigo não é o problema. O problema
é que quatro decisões de forma tomadas em março nunca tiveram a sua fase — e que
dois documentos canônicos deixaram de descrever a árvore. O primeiro custa
dinheiro por fase; o segundo custa o mapa.

---

## 4. Ordem de ação recomendada

Ordenada por razão valor/risco, não por gravidade. Os quatro primeiros itens não
abrem fase funcional.

### Passo 0 — Reconciliar o estado documentado (L-23, L-24) — **urgente**

Rodada documental. Zero código.

- Corrigir `docs/expandir.md`: o limite de 64 chamadas simultâneas não existe.
- Corrigir `docs/handoff_codex.md`: D2 foi entregue em 09/08/2026 pelo commit
  `1177077`; declarar qual é, de fato, a próxima prioridade funcional.
- Inventariar os 26 arquivos de teste com rótulos ausentes de `docs/` e dizer, de
  cada um, a que trabalho corresponde. Se algum for item de faixa do Bloco 20 —
  `d8_local_generic_inference_tests.rs` é o candidato mais provável —, isso muda o
  estado do roadmap.

Por que primeiro: é o único item cuja ausência faz todos os outros serem
priorizados sobre informação errada.

### Passo 1 — Gate de alcançabilidade e a poda que ele autoriza (L-01, L-14, L-16, L-17)

A recomendação original da série, inalterada. Acrescentar ao `pink nav verificar`
— que já roda em `make ci` — a exigência de que toda região cartografada com
símbolo `pub` tenha consumidor na árvore **ou** declare-se superfície pública
intencional. Depois, remover as ~275 linhas mortas de `backend_text` e **declarar
a intenção** de `palette`, `editor_tui` e `repl` em vez de apagá-las.

### Passo 2 — Duas correções baratas de risco latente (L-08, L-15 parcial)

- Teste que exija concordância entre `layout_of_type` e `struct_field_offsets`
  para todo struct dos exemplos versionados. Poucas dezenas de linhas; é
  pré-requisito honesto dos itens 25 e 26 da Faixa 7.
- Extrair a alocação de sufixo sintético de `lacos.rs` para um método único que
  incrementa e devolve, eliminando nove repetições manuais do mesmo par de linhas.

### Passo 3 — Duas decisões declaradas (L-06, L-02)

Não exigem código, exigem uma frase. Dizer o que `--asm-s` e `--pseudo-asm` são
hoje: superfícies validadas de produto, ou recortes históricos de inspeção. Hoje
são anunciadas como a primeira coisa e implementadas como a segunda.

### Passo 4 — Migrar `falar` para o registry (L-12) — **antes da Faixa 11**

Aplicar ao último enclave o tratamento que a consolidação C1 deu a sete
subsistemas. Reduz a superfície de vinte para algo em torno de oito a dez
arquivos. Fazer isso **durante** uma fase de I/O significaria pagar o pedágio de
vinte arquivos mais uma vez.

### Passo 5 — Tornar visível a fronteira linguagem/ferramental (L-19, L-22)

Declarar em `docs/code_map.md` quais módulos são pipeline e quais são
ferramental, e relatar as duas metades em separado no estado do projeto. Zero
código, e responde à pergunta "para onde está indo o esforço", que hoje só se
responde contando à mão.

### O que **não** fazer agora

L-03 (identidade por `String`), L-04 (`RuntimeValue`), L-05 e L-09 (modelos de
erro), L-10 (três enums de tipo), L-11 (layout de handles), L-13 (espaços de
handle) e a parte estrutural de L-15 (contrato único de iteração).

São reformas de porte. Cada uma merece fase própria, com critério de pronto,
paridade verificada e o padrão pós-Eixo B — exatamente o que `docs/expandir.md`
descreve. Nenhuma deve ser feita dentro de uma fase de feature, e nenhuma é
urgente hoje.

Duas delas, porém, têm prazo natural marcado no roadmap:

- **L-10 e L-15 vencem na Faixa 4** (item 16, tuplas; item 17, inferência de tipo
  local) e na **Faixa 6** (item 23, iteradores lazy). Três enums paralelos sem
  relação declarada não são um modelo em que inferência local se apoie.
- **L-08 vence na Faixa 7** (itens 25 e 26, packed structs e bitfields), que muda
  exatamente a regra de arredondamento duplicada.

A decisão de encará-las deve ser tomada **antes** de abrir essas fases, não
durante.

---

## 5. A lição que a série inteira sustenta

O Eixo B foi executado sob uma regra escrita: *"cada fase entrega a cobertura
completa do seu subproblema; nenhuma fase fecha no menor recorte auditável"*.

Seis meses depois, com 202.259 linhas auditadas linha a linha, **nenhum dos vinte
e quatro achados aponta para código entregue sob essa regra.**

Os achados apontam, quase todos, para o outro lado da linha: para as treze
primeiras janelas de PR, quando "mínimo" era o padrão automático — o mesmo
padrão que `docs/expandir.md` mediu do lado do processo e encontrou em 52,8% dos
títulos de fase.

Não é preciso reformar a Pinker. É preciso terminar de aplicar, aos pontos que
ficaram para trás, a política que o projeto já adotou e já provou que funciona.

---

## 6. Documentos da série

| Janela | Documento |
|---|---|
| PRs #1–50 | `auditoria-legado-inicial.md` |
| PRs #51–100 | `auditoria-legado-pr51-100.md` |
| PRs #101–150 | `auditoria-legado-pr101-150.md` |
| PRs #151–200 | `auditoria-legado-pr151-200.md` |
| PRs #201–250 | `auditoria-legado-pr201-250.md` |
| PRs #251–300 | `auditoria-legado-pr251-300.md` |
| PRs #301–350 | `auditoria-legado-pr301-350.md` |
| PRs #351–400 | `auditoria-legado-pr351-400.md` |
| PRs #401–450 | `auditoria-legado-pr401-450.md` |
| PRs #451–500 | `auditoria-legado-pr451-500.md` |
| PRs #501–550 | `auditoria-legado-pr501-550.md` |
| PRs #551–600 | `auditoria-legado-pr551-600.md` |
| PRs #601–611 | `auditoria-legado-pr601-611.md` |
| Síntese | este |
