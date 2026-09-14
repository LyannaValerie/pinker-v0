---
pinker-doc: 1
id: development.trama-utility-audit
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.trama-utility-audit
related:
  - development.trama
  - development.symbol-index
  - development.diff-coverage
  - development.projection-snapshots-contract
  - development.tramas-policy
---

# Auditoria de utilidade da Trama Pinker

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

Este documento registra uma auditoria medida da Trama Pinker do ponto de vista
de quem ela diz servir: um agente que precisa achar onde trabalhar e depois
trabalhar sem quebrar o repositório.

A auditoria **não** entrega nenhum item do Bloco 20 e **não** abre Fase, Faixa
ou Eixo. O item 16 da Faixa 4 (tuplas, `NÃO INICIADO`) foi usado apenas como
carga de trabalho realista: um construtor de tipo novo atravessa todo o
pipeline, então serve de sonda honesta. O mapa de implementação produzido no
caminho fica registrado aqui como subproduto reaproveitável, não como entrega.

<!-- @pinker-doc:start
id: development.trama-utility-audit.metodo
tags: [desenvolvimento, trama, auditoria, metodo, medicao]
aliases:
  - metodo da auditoria da trama
  - como a trama foi medida
summary: Método, oráculo e critérios de pontuação usados para medir a utilidade da Trama.
-->
## Método

Três experimentos, todos reproduzíveis no checkout:

1. **Recuperação.** Perguntas que um agente realmente faz, respondidas primeiro
   só com `pink nav` / `pink doc`, e depois com `grep`. O oráculo de cobertura é
   o conjunto de arquivos não-teste que mencionam o construtor de tipo mais
   recente (`uniao`, Fase 248), obtido por `grep -rl`.
2. **Escrita.** Uma linha inserida dentro de uma região indexada
   (`src/layout.rs`), seguida do ciclo completo até `make ci` verde, contando
   passos, tempo e ferramentas ausentes.
3. **Saúde do próprio sistema.** Conferência de quanto de cada capacidade
   publicada está de fato preenchida hoje na `main`.

Validação usada: `PINKER_EXIGE_NATIVO=1 make ci` com histórico completo. Os
gates da Trama (`docs-check`, `nav-check`, `change-history-check`, `guard`),
`fmt-check` e `clippy` passam. Seis testes de `automation_apply_tests` falham
neste contêiner por rodarem como `root` — eles esperam `IO_FAILURE` de um
ancestral sem permissão, e `root` ignora o bit; é ambiental e alheio a uma
mudança só documental.

Critério de pontuação por pergunta: **100% útil** = respondida só pela Trama;
**50%** = a Trama apontou a direção mas outro método fechou; **inútil** = outro
método respondeu mais rápido e melhor.
<!-- @pinker-doc:end development.trama-utility-audit.metodo -->

<!-- @pinker-doc:start
id: development.trama-utility-audit.resultados
tags: [desenvolvimento, trama, auditoria, resultados, recuperacao, escrita]
aliases:
  - resultados da auditoria da trama
  - o que a trama acerta
  - o que a trama erra
summary: Números medidos de recuperação, economia de contexto, custo de escrita e lacunas de preenchimento.
-->
## O que foi medido

### Economia de contexto — o argumento mais forte a favor

Responder "quais são as doze etapas do pipeline que um construtor de tipo novo
atravessa, e o que cada uma faz":

| Caminho | Bytes lidos |
|---|---|
| `pink nav mostrar <chave> --resumo` (12 regiões) | **6.746** |
| corpos das 12 regiões | 156.082 |
| os 12 arquivos inteiros | 1.064.370 |

**158× menos contexto** que abrir os arquivos, **23×** menos que ler só os
corpos. Os resumos conferidos por amostragem estavam corretos e declaravam
limites reais (`ir.tipos.conversao-ast` avisa que não reexecuta a checagem
semântica; `backend-s.abi.registradores-argumentos` avisa que não há alocação
dinâmica de registrador). Isso é valor que `grep` não produz em nenhuma
quantidade de chamadas.

### Recuperação — depende de já se conhecer o vocabulário do índice

Seis perguntas em linguagem natural, `pink nav buscar`, top-3:

| Pergunta | Resultado |
|---|---|
| onde um tipo novo entra na tabela de layout | acerto no 1º |
| onde o backend nativo escolhe registrador de argumento | acerto no 1º |
| onde fica a exaustividade do `encaixe` | parcial (devolveu o parser, não a semântica) |
| onde ficam os diagnósticos de erro semântico | erro (só regiões de evidência) |
| como adicionar uma intrínseca nova | erro |
| onde o interpretador guarda variáveis locais | erro (devolveu `semantic.escopos.variaveis`) |

Top-1 útil: **2 de 6**. Mas as respostas certas **estavam no índice**:
`interpreter.modelo.valores-estado` e `falha.operacional.superficies` aparecem
em primeiro lugar assim que a consulta usa as palavras do próprio índice
("modelo valores estado interpretador", "falha operacional"). O problema é de
**recuperação, não de cobertura**: a Trama sabe, mas exige que se pergunte com
as palavras dela.

`pink doc rota` é mais fraco que `pink doc buscar`: das três intenções testadas,
`rota` devolveu vazio em duas e resposta irrelevante na terceira, enquanto
`buscar` acertou a pergunta do roadmap.

### Cobertura de blast radius — a Trama sozinha não fecha

Oráculo: 36 arquivos não-teste mencionam o construtor `uniao`.

| Método | Arquivos achados | Recall | Chamadas |
|---|---|---|---|
| `nav listar unioes` | 5 | 14% | 1 |
| `nav listar` em 6 domínios relevantes | 13 (2 fora do alvo) | 36% | 6 |
| `grep -rl` | 36 | 100% | 1 |

A leitura correta não é "a Trama perde". Os 13 que ela acha são os
**conceitualmente centrais**, cada um com resumo; os 23 que faltam são em boa
parte espelhos mecânicos (`*_validate.rs`, `render`, `instr_select`) que o
`grep` acha de graça. Os dois métodos são complementares, e nenhum dos dois
sozinho é suficiente.

### Custo de escrita — real, mecânico e não documentado

Uma linha inserida dentro de `layout.tipos.memoria`:

- `pink nav verificar` → `E-NAV-VERIFY`, exit 5 — mas a mensagem diz só
  "catálogo dessincronizado", **sem nomear arquivo ou região**;
- `pink nav sincronizar` → 1,4 s, diff de **uma linha** no catálogo. Excelente:
  o índice não faz churn;
- `pink nav projecao verificar` → **13 de 13 snapshots `FROZEN` em
  `HARNESS_FAILURE`** (`OverrideStaleBase`), exit 6;
- `make ci` → **9 testes falham** em `nav_cartography_tests`.

O conserto é uma edição manual de **uma linha** (`from = "fnv1a64:..."` no
snapshot), e o diagnóstico já entrega o hash esperado e o encontrado. Ciclo
completo até verde: **5 passos, ~2 s**, confirmado com
`cargo test --test nav_cartography_tests` → 35 passed, 0 failed. O histórico da
`main` mostra que é a prática normal: os commits `0450188`, `4e146a3`, `5ed8e24`
e `7b9ab4f` reancoram de 2 a 13 linhas de projeção junto com o código, e
`348117f` existe só para isso.

O problema **não é o custo** — é que ele é indescobrível:

- `CONTRIBUTING.md` não menciona projeções, snapshots ou reancoragem;
- `pink nav projecao` não tem subcomando de reancoragem (`listar`, `mostrar`,
  `verificar`, `preparar`, `aceitar` — os dois últimos são de `CANDIDATE`);
- `AGENTS.md` diz *"Nunca recalibre `regions`, `length`, `fnv1a64` ou a projeção
  estável para esconder drift"*. A frase é sobre esconder drift, mas um agente
  que acabou de ler `OverrideStaleBase` numa linha literalmente escrita
  `from = "fnv1a64:..."` tem toda razão para se recusar a consertar.

Superfície afetada: **124 regiões (20%)** têm base de hash dura em algum
snapshot `FROZEN`. Delas, **31 dos 36 arquivos** do mapa do item 16 contêm ao
menos uma — cerca de **53% das linhas** daquele território.

### Custo de escrita documental — barato, mas o portal não é conferido

Acrescentar este próprio documento mediu o outro lado da escrita:

- `pink doc verificar` acusou `E-DOC-VERIFY` e apontou o comando certo;
- `pink doc sincronizar` levou **31 ms** e acrescentou **5 linhas** ao catálogo;
- nenhuma projeção quebrou, e `nav projecao verificar` seguiu `MATCH`.

Um acerto lateral: `make change-history-check` recusa clone raso com
`E-CHANGE-HISTORY-SHALLOW-CLONE` e já entrega o comando do conserto
(`git fetch --unshallow`). É o oposto do `E-NAV-VERIFY` mudo — mostra que o
projeto sabe escrever bom diagnóstico quando decide escrever.

A seção nova ficou consultável imediatamente por `pink doc buscar` e
`pink doc mostrar`. Mas o portal `docs/development/README.md` mantém **à mão**
uma lista de documentos e uma tabela de rotas, e `doc verificar` passou verde
com o documento novo ausente das duas. O catálogo de máquina e o portal humano
podem divergir em silêncio.

### Preenchimento — metade do sistema está parada

| Capacidade | Contrato | Estado medido |
|---|---|---|
| Catálogo de código | regiões derivadas de marcadores | **613 regiões, 78% das linhas não-teste, 98/127 arquivos** — vivo e sincronizado |
| Índice de símbolos (`nav localizar`) | identidade estrutural e vínculos | **13 bindings, 7 símbolos, 9 de 613 regiões (1,5%)** — e **todos os 7 são da própria Trama** (`nav`, `doc_index`, `symbol_index`, `diff_coverage`). Zero símbolos do compilador. `nav localizar Type` e `nav localizar TypeIR` devolvem vazio |
| Manifestos de mudança | "fonte para geração derivada" | até o cutover (PR #410) a cobertura é **completa e conferida por teste**; depois dele, **107 merges, 18 com manifesto, 89 sem** — a lacuna é contínua do #441 ao #673 |
| Projeções documentais geradas | tabela mecânica de entregas | `docs/roadmap/generated.md` e `docs/history/changes.md` **param no #440** |
| Verificação | `nav verificar`, `doc verificar` | verdes, ~1,5 s — barato e confiável |

A lacuna **não é violação de contrato**: `change_history_coverage_tests` só
exige manifesto ou exceção registrada para merges **até o cutover**
(`cutover_merge_sha = 1df2a6a`, o merge do PR #410), e esse gate passa. Depois
do cutover nenhum teste cobre a persistência.

O problema é outro, e é de dono. O `trama.yml` **exige** o bloco
`pinker-change` em todo PR posterior ao marco #330 e o valida com
`doc importar-pr --check`, que por desenho **valida sem escrever**. Nada nem
ninguém roda o modo de escrita depois do merge. O resultado é que os autores
continuam pagando o custo de preencher o bloco em 89 PRs cujo conteúdo nunca
virou manifesto, e `docs/roadmap/generated.md` publica como corrente um ledger
que termina no #440.
<!-- @pinker-doc:end development.trama-utility-audit.resultados -->

<!-- @pinker-doc:start
id: development.trama-utility-audit.mapa-item-16
tags: [desenvolvimento, trama, roadmap, tuplas, item-16, mapa]
aliases:
  - mapa do item 16
  - onde mexer para tuplas
  - blast radius de construtor de tipo
summary: Mapa de doze etapas produzido pela Trama para o item 16 do Bloco 20, com regiões, arquivos e exposição a snapshots congelados.
-->
## Subproduto: mapa do item 16 (tuplas)

Produzido **pela Trama**, verificado contra o oráculo. Não é entrega do item.

| Etapa | Região da Trama | Arquivo | Linhas |
|---|---|---|---|
| lexer / token | `token.representacao.spans` | `src/token.rs` | 110-231 |
| gramática de tipos | `parser.tipos.gramatica` | `src/parser/mod.rs` | 981-1286 |
| AST | `ast.tipos.representacao` | `src/ast.rs` | 437-797 |
| sistema de tipos | `semantic.tipos.sistema` | `src/semantic.rs` | 592-1266 |
| layout | `layout.tipos.memoria` | `src/layout.rs` | 8-230 |
| IR | `ir.tipos.conversao-ast` | `src/ir.rs` | 877-1030 |
| validação CFG | `cfg.unioes.validacao-operacoes` | `src/cfg_ir_validate.rs` | 31-68 |
| canonicalização | `union.unioes.canonicalizacao` | `src/union_canon.rs` | 31-508 |
| payload / representação | `uniao.payload.classificacao` | `src/union_payload.rs` | 24-361 |
| interpretador | `interpreter.modelo.valores-estado` | `src/interpreter.rs` | 157-1163 |
| ABI nativa | `backend-s.abi.registradores-argumentos` | `src/backend_s.rs` | 187-192 |
| runtime | `runtime.unioes.descritor` | `runtime/pinker_rt/src/lib.rs` | 1876-2233 |

Os 23 arquivos que a Trama não nomeia são, em quase todos os casos, os espelhos
de validação e renderização das mesmas etapas. Quem for implementar o item 16
deve prever a reancoragem de base em 31 dos 36 arquivos.
<!-- @pinker-doc:end development.trama-utility-audit.mapa-item-16 -->

<!-- @pinker-doc:start
id: development.trama-utility-audit.veredito
tags: [desenvolvimento, trama, auditoria, veredito, recomendacao]
aliases:
  - vale a pena manter a trama
  - veredito da auditoria
summary: Veredito por capacidade e os quatro consertos baratos que resolvem a maior parte do custo medido.
-->
## Veredito por capacidade

| Capacidade | Veredito |
|---|---|
| Resumos de região + `--resumo` | **manter** — 158× de economia de contexto, sem substituto |
| `nav mostrar` / `nav listar` / `nav mapa` | **manter** — preciso e barato quando se sabe a chave |
| `nav buscar` | **manter e melhorar recuperação** — o conteúdo está lá; o ranking não entrega |
| `nav verificar` / `doc verificar` / `sincronizar` | **manter** — 1,5 s, diff mínimo, sem churn |
| `doc mostrar` / `doc buscar` | **manter** |
| `doc rota` | **rever** — perde para `doc buscar` nas mesmas intenções |
| Snapshots `FROZEN` | **manter o mecanismo, documentar a reancoragem** — o custo é 1 linha, o problema é ninguém saber disso |
| `nav localizar` (índice de símbolos) | **preencher ou remover** — hoje indexa 7 símbolos, todos da própria Trama |
| Manifestos `.pinker/changes` + projeções geradas | **religar ou parar de exigir o bloco** — o bloco é cobrado em todo PR, mas 89 dos 107 merges pós-cutover nunca viraram manifesto |

## Recomendação

**Não remover.** A parte cara (marcadores e catálogo) é a parte que funciona, e
a economia de contexto é grande demais para descartar. O custo real não está na
infraestrutura: está em duas capacidades que foram construídas e nunca
preenchidas, e numa manobra obrigatória que não está escrita em lugar nenhum.

Quatro consertos baratos resolvem a maior parte do custo medido:

1. escrever a reancoragem de base em `CONTRIBUTING.md` e no `AGENTS.md`, e
   desfazer a leitura de que ela é proibida;
2. fazer `pink nav verificar` **nomear a região** que dessincronizou;
3. decidir o destino de `nav localizar` — ou preencher os símbolos do
   compilador, ou retirar o comando;
4. decidir o destino do ledger — ou dar dono ao passo de escrita
   (`doc importar-pr` sem `--check`, após o merge), ou parar de cobrar o bloco
   `pinker-change` e de publicar `docs/roadmap/generated.md` como corrente.
<!-- @pinker-doc:end development.trama-utility-audit.veredito -->
