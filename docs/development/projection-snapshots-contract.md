---
pinker-doc: 1
id: development.projection-snapshots-contract
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.projection-snapshots-contract
related:
  - development.deterministic-infrastructure-window
  - development
---

# Contrato dos snapshots históricos de projeção

- **Classe:** Engine
- **Papel:** contrato de domínio
- **Status:** ativo

Este documento descreve o contrato dos snapshots históricos das projeções do
catálogo de navegação, implementado em `src/nav_projection_snapshot.rs` sob a
segunda capacidade da janela auxiliar
(`janela-infraestrutura-deterministica.md`, Issue #384).

A implementação possui agora duas fronteiras explícitas. O resolvedor e o store
continuam somente leitura; o lifecycle calcula estado desejado e delega toda
observação, autorização, proteção stale e escrita ao automation core. O acervo
continua com 13 snapshots FROZEN e 1 receita histórica em
`.pinker/projections/`: implementar o lifecycle não registra um marco real.

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.schema
tags: [desenvolvimento, snapshots, projecoes, determinismo, cartografia]
aliases:
  - snapshot de projecao
  - schema de snapshot historico
  - medidas historicas da cartografia
summary: Schema versionado, identidade, estados, medidas e regras de reconstrução dos snapshots históricos de projeção do catálogo de navegação.
-->
## Domínio e fronteira

`src/projection.rs` é a autoridade das projeções **documentais**: ele projeta os
manifestos versionados em regiões geradas de documentos humanos (§12). Os
snapshots históricos são **outro domínio**: congelam medidas de uma projeção
estável de regiões do catálogo de código. A coincidência da palavra "projeção" é
terminológica; os dois módulos não compartilham modelo, formato nem regras.

## Projeção estável

A projeção estável de um conjunto de regiões contém apenas campos que não mudam
com edições irrelevantes: schema, `key`, `kind`, `domain`, `layer`, `file`,
`summary`, `hash` e `status`. Um registro por região, ordenado
lexicograficamente e concatenado. Números de linha ficam de fora de propósito.

Como `file` já é repo-relativo, a projeção é idêntica em qualquer root absoluto.

## Medidas

Um snapshot congela exatamente três medidas da projeção estável:

| Medida | Significado |
|---|---|
| `regions` | quantidade de regiões |
| `length` | comprimento em bytes da projeção |
| `fnv1a64` | FNV-1a 64 da projeção, em `fnv1a64:` + 16 hexadecimais minúsculos |

## Schema versionado

Formato TOML, um arquivo por snapshot, em `.pinker/projections/<id>.toml`. O
exemplo abaixo usa `schema = 1` porque é a forma mínima do formato — lista plana,
sem composição. **Artefatos novos são emitidos em `schema = 3`**; as versões
anteriores seguem aceitas para leitura.

```toml
schema = 1
id = "identificador-estavel"
state = "FROZEN"
predecessor = "identificador-anterior"
justification = "por que este snapshot existe"

[reconstruction]
expected_overrides = 1
expected_exclusions = 2

[measures]
regions = 2
length = 302
fnv1a64 = "fnv1a64:0123456789abcdef"

[[rules]]
op = "exclude-key"
key = "posterior.nova"
expected_matches = 1

[[rules]]
op = "exclude-file-prefix"
prefix = "apps/"
expected_matches = 2

[[rules]]
op = "override-hash"
key = "camada.dominio.regiao"
from = "fnv1a64:0000000000000001"
to = "fnv1a64:00000000000000ff"
expect_file = "src/exemplo.rs"
expect_domain = "dominio"
expect_layer = "camada"
```

O parser é estrito. Rejeita schema ausente ou desconhecido, chave desconhecida,
chave duplicada, seção duplicada, estado desconhecido, identificador inseguro,
hash malformado, número negativo, overflow, predecessor igual ao próprio
identificador, path absoluto, travessia, regra sem operação, regra sem seletor,
dado residual após o valor, string incompleta e escape não suportado.

O renderer é canônico: ordem fixa de campos e seções, regras ordenadas por
operação e seletor. `parse(render(x))` devolve `x` e `render` é idempotente. A
saída não depende de root absoluto, PID, usuário, locale, tempo, iteração de
tabela hash ou endereço de memória.

## Estados

Só existem dois. `FROZEN` é byte-imutável e nunca é recalibrado.
`CANDIDATE` é uma proposta versionada de novo marco, preparada e aceita somente
por comandos explícitos. Os 13 snapshots materializados continuam todos
`FROZEN`; candidates usados por testes vivem apenas em fixtures temporárias.
<!-- @pinker-doc:end development.projection-snapshots-contract.schema -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.reconstrucao
tags: [desenvolvimento, snapshots, reconstrucao, overrides, drift]
aliases:
  - regras de reconstrucao
  - consumo de overrides
  - drift versus harness failure
summary: Regras de reconstrução com orçamento explícito de consumo e a separação entre MATCH, DRIFT e HARNESS_FAILURE.
-->
## Reconstrução e consumo

Reconstruir é recompor, a partir do catálogo corrente, o estado histórico que
produziu as medidas congeladas. As exclusões são aplicadas primeiro, depois os
overrides; a ordem textual das regras não afeta o resultado.

Toda regra declara o quanto deve consumir, e o consumo é verificado:

- `override-hash` consome exatamente uma correspondência;
- `exclude-key` e `exclude-file-prefix` consomem exatamente
  `expected_matches`, que precisa ser ao menos `1`;
- `expected_overrides` e `expected_exclusions` fixam a quantidade de regras de
  cada família, de modo que uma regra removida ou acrescentada falhe antes de
  qualquer medição.

Falham como erro fatal: override ausente, excedente, repetido ou não consumido;
seletor ambíguo; região removida; key alterada; path alterado; metadata
alterada; base divergente do `from` declarado; exclusão sem correspondência; e
exclusão parcialmente consumida.

## Resultados

| Resultado | Quando |
|---|---|
| `MATCH` | reconstrução válida e as três medidas coincidem |
| `DRIFT` | reconstrução válida e ao menos uma medida diverge |
| `HARNESS_FAILURE` | a reconstrução não pôde ser concluída |

Drift só existe **depois** de uma reconstrução válida. Uma falha de harness nunca
é reclassificada como drift, e um relatório de falha não carrega medida
observada: sem reconstrução válida não há o que observar.

Os relatórios são determinísticos, derivados do mesmo modelo, sem códigos ANSI e
sem qualquer path absoluto.

## Lifecycle explícito de CANDIDATE

A superfície final é `pink nav projecao`, com cinco comandos:

```text
listar
mostrar <id> [--observado]
verificar [<id>]
preparar <id> --justificativa <texto> --predecessor <id> [--autorizar <digest>]
aceitar <id> [--autorizar <digest>]
```

`listar` inventaria definições sem reconstruí-las. `mostrar` separa sempre
`definicao` de `observado`; medidas congeladas e observadas nunca ocupam o mesmo
objeto ambíguo. `verificar` usa a composição real (`base_snapshot`, recipes e
regras locais), continua depois de falhas independentes e agrupa um drift de
base com os descendentes bloqueados por `BaseMeasuresDiverged`.

### Preparar

`--predecessor` é obrigatório, não inferido, e precisa nomear um FROZEN distinto
do candidate. Um candidate novo é uma raiz (`base_snapshot` ausente), carrega
justificativa não vazia, zero regras e a recipe própria
`normalizacao-corrente-para-<id>`. As três medidas são calculadas pelo resolvedor
real sobre o catálogo corrente; nunca entram como argumento da CLI.

A recipe própria nasce em
`.pinker/projections/recipes/normalizacao-corrente-para-<id>.toml`, vazia e na
forma de `render_recipe()`. Repreparar um CANDIDATE recalcula seu snapshot. A
recipe ausente é criada; uma recipe vazia canônica é preservada byte a byte;
qualquer step, regra, contador, id ou forma canônica divergente é violação de
política. O lifecycle nunca sobrescreve manutenção semântica humana. Preparar
um ID já FROZEN também é violação de política.

Sem `--autorizar`, o comando recalcula o estado, observa os dois targets,
executa check e publica somente resumo e digest. Com autorização, todo o cálculo
é repetido; o plano anterior não é lido nem desserializado. A allowlist contém
exatamente o snapshot e sua recipe. Atomicidade é por arquivo: progresso parcial
fica explícito e o rerun converge por nova observação e novo digest, sem rollback
global.

### Aceitar

Aceitar exige candidate e recipe próprios válidos e canônicos, predecessor
FROZEN, biblioteca válida, dependências FROZEN seguras, reconstrução válida e
medidas `MATCH`. Harness failure sai antes de qualquer plano; reconstrução válida
com medidas divergentes é `DRIFT`.

A transição possui exatamente um target e muda semanticamente somente
`state = "CANDIDATE"` para `state = "FROZEN"` no mesmo arquivo. ID,
predecessor, justificativa, base, recipes, contadores, regras e medidas são
preservados. Aceitar não cria nem modifica recipe e não recalcula nada. Depois
do apply, o arquivo é reaberto, reparseado e resolvido; a recipe é comparada byte
a byte. Uma segunda aceitação é recusada por lifecycle e não produz delta.

### Automation core, exits e schemas

O adaptador de domínio não escreve diretamente. Descoberta de root,
`RelativePath`, allowlist, plano, observação, check, digest, autorização, apply,
stale protection, escrita atômica por arquivo e `ApplyReport` pertencem ao
automation core.

| exit | significado |
|---:|---|
| 0 | `MATCH`, `NO_CHANGE`, planos, candidate preparado ou FROZEN aceito |
| 1 | I/O ou falha de verificação pós-apply |
| 2 | uso inválido |
| 3 | autoridade ou catálogo ilegível |
| 4 | ID inexistente |
| 5 | `DRIFT` |
| 6 | `HARNESS_FAILURE` |
| 7 | `POLICY_VIOLATION` |
| 8 | `STALE_PLAN` |

O JSON de `pink nav projecao` usa `PROJECTION_CLI_SCHEMA = 1`. O relatório
histórico de `json_report()` usa `SNAPSHOT_REPORT_SCHEMA = 1`. Ambos são
protocolos de relatório, separados do `SNAPSHOT_SCHEMA = 3` do artefato TOML.
Saídas são de uma linha, determinísticas, sem ANSI, root absoluto, timestamp,
PID ou payload completo do plano.

### Evolução futura

Recipes não recebem lifecycle. Quando o catálogo evolui, manutenção semântica
é edição humana deliberada: regras estritas e, quando apropriado, composição por
`steps`. Nenhum FROZEN ou medida é recalibrado. Recipe consegue excluir regiões
posteriores e adaptar campos com precondições; não consegue fabricar região
histórica removida. Falha de consumo continua `HARNESS_FAILURE`, nunca drift.

Continuam fora deste contrato: comando de manutenção automática de recipes,
transação multi-arquivo, rollback global, novo executor e qualquer alteração de
`pink-agent-v1`.
<!-- @pinker-doc:end development.projection-snapshots-contract.reconstrucao -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.composicao
tags: [desenvolvimento, snapshots, composicao, receitas, schema]
aliases:
  - schema 2 e 3 de snapshot
  - receita de reconstrucao
  - composicao de reconstrucao
  - autoridade historica materializada
summary: Schemas 2 e 3 do snapshot, formato de receita, invariantes do grafo de composição e o acervo histórico materializado da Issue #384.
-->
## Quantas receitas a migração criou

**Uma.** E o número não veio de contar helpers: veio de perguntar quais
transformações têm significado próprio e são reutilizadas.

A pergunta original desta seção supunha que cada helper legado viraria uma
receita. Essa suposição não sobreviveu à migração, por um motivo que vale
registrar: o helper legado é um programa **tolerante** — um `if` que não casa
segue adiante, um `retain` sobre chave ausente é no-op — enquanto a autoridade
canônica é **estrita**, e toda regra precisa consumir. Traduzir estrutura
produziria regras que abortam.

A unidade de tradução passou a ser o delta observado:

```
estado de entrada + delta efetivamente ocorrido = estado reconstruído
```

Das 818 operações candidatas observadas nos caminhos legados, **287 produziram
efeito**. As outras não existem como transformação: 481 ramos dormentes, 19
overrides sobre região já removida e 31 exclusões que não consomem nada. Um ramo
que nunca alterou coisa alguma não é regra — é linha de código.

Sobrou exatamente uma transformação com identidade própria e reutilização real:
retirar do catálogo corrente as regiões posteriores a todo o acervo histórico.
Ela é a receita `normalizacao-corrente-para-historico`.

Um experimento de fatoração mostrou que cinco regras de exclusão se repetem em
contextos aninhados e poderiam virar cinco receitas de uma regra cada. Foi
**rejeitado**: deduplicar declarações não é motivo para criar autoridade
nomeada. Receita representa transformação reutilizável **semanticamente**, não
economia textual.

## Por que existe composição

O inventário refeito sobre a `main` mostrou helpers de reconstrução dispostos num
DAG e apenas 13 estados com medida histórica própria. Vários helpers são nós
puramente intermediários: nenhum teste os chama e nenhum produz medida. (A
contagem exata foi corrigida depois: a enumeração original filtrava por prefixo
de nome e perdia um helper cujo nome fugia da convenção. Enumerar pela
assinatura, não pelo nome, deu 13 helpers históricos e 4 posteriores ao acervo.)

Achatar as cadeias produziria arquivos de noventa regras sem relação visível com
a estrutura real. Inventar snapshots para os oito nós seria fabricar história.
Nenhuma das duas é aceitável, então o formato ganhou composição — e uma segunda
autoridade, deliberadamente menor.

## Duas autoridades

| | snapshot | receita |
|---|---|---|
| local | `.pinker/projections/<id>.toml` | `.pinker/projections/recipes/<id>.toml` |
| medidas | sim | **não** |
| estado | sim | **não** |
| predecessor | sim | **não** |
| compõe | `base_snapshot` + `recipes` | apenas `recipes` |
| versões aceitas | 1, 2, 3 | 1, 2 |
| versão emitida hoje | `schema = 3` | `schema = 2` |

Os formatos são versionados de forma independente e evoluíram em ritmos
diferentes. O snapshot ganhou composição no `schema = 2` e `override-region` no
`schema = 3`; a receita estreou em 1 e ganhou `override-region` em 2. Cada
autoridade sobe de versão quando **ela** ganha capacidade, não quando a outra
sobe.

Versão aceita e versão emitida são coisas distintas: o parser continua lendo
todas as anteriores, e nenhum artefato existente é reescrito por causa de um
bump. Artefatos novos, porém, nascem na versão corrente do respectivo formato —
não na mínima que aquele caso específico precisaria.

Uma receita que declare `state`, `predecessor`, `justification`, `measures`,
`base_snapshot` ou `recipes` é rejeitada com erro **nomeado**, não com "chave
desconhecida": a ausência desses campos é estrutural, e o diagnóstico diz isso.

## Duas relações distintas

| Relação | Significa |
|---|---|
| `predecessor` | o snapshot histórico **anterior na linha do tempo** |
| `reconstruction.base_snapshot` | o estado sobre o qual **esta reconstrução se apoia** |

Elas coincidem em alguns snapshots e divergem em outros. Tratá-las como a mesma
coisa perderia a distinção.

## Namespaces resolvem estruturalmente

`base_snapshot` procura somente entre snapshots. `recipes` e `steps` procuram
somente entre receitas. Não há resolvedor polimórfico, então um snapshot e uma
receita podem compartilhar o mesmo identificador textual — e **não existe falha
de base ambígua**, porque a ambiguidade nunca foi introduzida.

Uma receita não pode depender de snapshot. O grafo tem uma direção só:

```text
snapshot → snapshot
         → receita → receita
```

## Ordem de aplicação

1. resolve a base recursivamente **e verifica as medidas dela**;
2. aplica as receitas na ordem declarada, cada uma resolvendo seus próprios
   passos antes das próprias regras;
3. aplica as exclusões locais;
4. aplica os overrides locais.

A ordem de `recipes` e de `steps` é **procedural**: o renderer a preserva em vez
de canonicalizar por nome. Regras locais, que são independentes entre si, seguem
a canonicalização de sempre.

## A base é verificada, não apenas reconstruída

Cada `base_snapshot` é conferido contra as **próprias** medidas congeladas antes
de servir de fundação. Sem isso, uma base quebrada poderia ser compensada por
coincidência pelas regras do descendente, e a separação entre erro de harness e
drift — que é o ponto da Issue #384 — deixaria de valer. Regras do descendente
nunca mascaram falha da base.

## Invariantes do grafo

| Situação | Resultado |
|---|---|
| `base_snapshot` inexistente | `HARNESS_FAILURE` |
| receita inexistente | `HARNESS_FAILURE` |
| autorreferência | `HARNESS_FAILURE` |
| ciclo, no grafo completo | `HARNESS_FAILURE` |
| receita repetida no mesmo escopo | `HARNESS_FAILURE` |
| base que não bate com as próprias medidas | `HARNESS_FAILURE` |
| `FROZEN` dependendo de `CANDIDATE`, direta **ou transitivamente** | `HARNESS_FAILURE` |

Receitas não têm estado, então são neutras na última regra — consequência de não
lhes darmos estado, não exceção.

O consumo é validado **em cada escopo**, e nenhum consumo é contado duas vezes:
cada regra pertence a exatamente um escopo, e o ledger registra a sequência
`recipe:… → snapshot:…` na ordem de aplicação.

## Diagnósticos são separados por autoridade

As duas autoridades têm conjuntos de versão diferentes, e o erro diz de qual
formato fala:

| Situação | Código | Mensagem |
|---|---|---|
| snapshot com versão fora de {1, 2, 3} | `E-SNAP-SCHEMA` | "schema N desconhecido para snapshot; este formato aceita 1, 2 ou 3" |
| receita com versão fora de {1, 2} | `E-RECEITA-SCHEMA` | "schema N desconhecido para receita; este formato aceita 1 ou 2" |

O mesmo vale para autorreferência, porque a relação não é a mesma nas duas:

| Situação | Código | Mensagem |
|---|---|---|
| snapshot com `base_snapshot` igual ao próprio ID | `E-SNAP-BASE-PROPRIA` | "snapshot 'x' declara a si mesmo como base" |
| receita com passo igual ao próprio ID | `E-RECEITA-PASSO-PROPRIO` | "receita 'x' declara a si mesma como passo" |

Uma receita não tem base. Dizer que ela "declarou a si mesma como base"
descreveria uma relação que não existe naquela autoridade, e mandaria quem lê o
log procurar um campo que o formato não tem.

## O schema 1 continua significando o que significava

Um arquivo que declara `schema = 1` continua sendo lista plana. Usar
`base_snapshot`, `recipes`, `exclude-file` ou `exclude-key-prefix` nele é falha
explícita — `E-SNAP-CAPACIDADE-SCHEMA` — e nunca interpretação silenciosa.

## Matriz de capacidades

A versão mínima de cada operação depende da **autoridade**: os dois formatos
evoluíram em ritmos diferentes. `exclude-file` e `exclude-key-prefix` chegaram ao
snapshot no schema 2, mas o formato de receita nasceu depois e já as trouxe na
primeira versão.

| Operação | snapshot | receita |
|---|---:|---:|
| `override-hash` | 1 | 1 |
| `exclude-key` | 1 | 1 |
| `exclude-file-prefix` | 1 | 1 |
| `exclude-file` | 2 | 1 |
| `exclude-key-prefix` | 2 | 1 |
| `override-region` | 3 | 2 |

Usar uma capacidade acima da versão declarada é `E-SNAP-CAPACIDADE-SCHEMA` ou
`E-RECEITA-CAPACIDADE-SCHEMA`, e o diagnóstico carrega autoridade, capacidade,
versão encontrada e versão exigida — não uma regra fixa numa versão específica.

## Estriteza por operação

A gramática tem dois filtros, e eles respondem por coisas diferentes:

1. **união** — a chave existe em algum lugar do formato?
2. **por operação** — a chave pertence a **esta** regra?

Sem o segundo, um campo legítimo de outra operação passava pelo primeiro e era
descartado em silêncio: `from_summary` numa regra `override-hash` era aceito e
ignorado, inclusive em versões que nem conhecem `override-region`. Quem escreveu
a linha acreditava ter declarado uma restauração que não existia.

Os campos permitidos vivem numa tabela única, por operação:

| Operação | Campos |
|---|---|
| `override-hash` | `op`, `key`, `from`, `to`, `expect_file`, `expect_domain`, `expect_layer` |
| `override-region` | `op`, `key`, `from_hash`, `to_hash`, `from_summary`, `to_summary`, `expect_file`, `expect_domain`, `expect_layer` |
| `exclude-key` | `op`, `key`, `expected_matches` |
| `exclude-key-prefix` | `op`, `prefix`, `expected_matches` |
| `exclude-file` | `op`, `file`, `expected_matches` |
| `exclude-file-prefix` | `op`, `prefix`, `expected_matches` |

Acrescentar capacidade a uma operação é editar uma linha dessa tabela — não
lembrar de um `if` espalhado pelo braço correspondente.

Os dois diagnósticos permanecem distinguíveis:

| Situação | Erro |
|---|---|
| chave que nenhuma operação conhece | campo desconhecido |
| chave de outra operação | `E-SNAP-CAMPO-DA-OPERACAO` — "campo 'x' não pertence à operação 'y'" |

## `override-region`

A reconstrução histórica real restaura `region.summary`, e `summary` participa da
projeção estável. O schema 2 só sabia alterar `hash`: ele não conseguia
representar a própria história que deveria migrar. `override-region` existe para
isso, e **apenas** para isso.

Restaura, de uma única região selecionada por `key`, somente:

- `hash`
- `summary`

Não é uma operação genérica de campo. A allowlist é essa, e é fechada.

```toml
[[rules]]
op = "override-region"
key = "camada.dominio.regiao"

from_hash = "fnv1a64:0000000000000001"
to_hash = "fnv1a64:00000000000000ff"

from_summary = "texto corrente"
to_summary = "texto histórico"

expect_file = "src/exemplo.rs"
expect_domain = "dominio"
expect_layer = "camada"
```

### Pares

Cada par `from`/`to` é individualmente opcional, mas:

- ao menos um par completo precisa existir;
- `from_hash` sem `to_hash`, ou o inverso, é inválido;
- `from_summary` sem `to_summary`, ou o inverso, é inválido.

Meio par não descreve restauração: um `from` sozinho não muda nada, e um `to`
sozinho seria mutação sem precondição.

### Atômica no sentido lógico da regra

A aplicação tem duas fases, nesta ordem:

1. **validação** — identidade (`expect_file`, `expect_domain`, `expect_layer`) e
   **todos** os `from` declarados;
2. **mutação** — só se a primeira fase inteira passar.

Uma regra que restaura dois campos nunca deixa metade aplicada. Se o hash
confere e o summary não, nada é tocado; e vice-versa. Qualquer divergência é
`HARNESS_FAILURE`, nunca drift.

### Consumo

Uma regra bem-sucedida consome exatamente uma região. `override-region` conta
como **uma** regra de override em `expected_overrides`, independentemente de
restaurar um ou dois campos — o orçamento é por regra, não por campo. Duas
regras de override para a mesma `key` continuam proibidas no mesmo escopo,
inclusive misturando `override-hash` e `override-region`.

### Quando usar cada uma

`override-hash` continua existindo com exatamente a semântica anterior. Use-a
para restaurações exclusivamente de hash, quando isso representar naturalmente o
legado. Use `override-region` quando houver restauração de `summary`, isolada ou
junto de `hash`.

## A autoridade histórica materializada

A Issue #384 materializou 13 snapshots e 1 receita em `.pinker/projections/`.

### Identificadores

Os identificadores vêm da **identidade histórica** do marco que cada estado
representa — o gate que o mede — e nunca de uma medida. Um identificador que
carregasse `regions`, `length` ou o FNV mudaria de nome sempre que a medida
fosse recalculada, o que é exatamente o oposto de identidade.

| id canônico | significado histórico |
|---|---|
| `onda-8f-anterior` | estado anterior à evidência da Onda 8F (backend textual) |
| `onda-8g-anterior` | estado anterior à evidência da Onda 8G (backend-s textual) |
| `onda-8h-anterior` | estado anterior à evidência da Onda 8H (toolchain externa) |
| `onda-8i-anterior` | estado anterior à evidência da Onda 8I (backend nativo) |
| `onda-8j-anterior` | estado anterior à evidência da Onda 8J (runtime interno) |
| `onda-8-convergencia` | conjunto convergido da Onda 8 |
| `capsula-nav-catalog` | estado completo da cápsula nav-catalog |
| `capsula-doc-catalog` | estado completo da cápsula doc-catalog |
| `capsula-trama-query` | estado completo da cápsula trama-query |
| `onda-pink-agente-a` | Onda A do agente Pinker |
| `onda-pink-agente-b` | Onda B do agente Pinker |
| `onda-pink-agente-c` | Onda C do agente Pinker |
| `onda-pink-agente-d` | Onda D do agente Pinker |

### Seis eras, treze snapshots

Os 13 estados se organizam em **6 eras temporais**. A primeira era concentra
oito deles: são recortes de escopo do mesmo momento histórico, medidos por gates
diferentes, cada um excluindo as regiões da própria evidência. Eras não reduzem
a contagem de snapshots — um recorte com medida própria é um estado próprio.

Isso é o que separa as duas relações na prática: dentro da primeira era todos
compartilham `base_snapshot` e nenhum tem `predecessor`, porque não houve
mudança de era entre eles.

### A receita de normalização

`normalizacao-corrente-para-historico` remove as 33 regiões acrescentadas depois
de todo o acervo histórico (534 → 501 regiões) e restaura estritamente dez
regiões existentes cujo corpo ou resumo mudou ao integrar o Stage E e a consulta
consolidada da Issue #387. Ela é
declarada por **um único snapshot**, o terminal, cuja reconstrução parte do
catálogo corrente. Os demais herdam o efeito por `base_snapshot`; reaplicá-la
abortaria por consumo zero, e a composição simplesmente não tenta.

### Proveniência das medidas

As três medidas de todos os 13 snapshots são **literais históricos migrados**:
`regions`, `length` e `fnv1a64` já existiam no legado. Nenhuma foi derivada da
reconstrução.

O registro anterior falava em nove contagens derivadas. Esse número vinha de um
inventário que só inspecionava a tupla do assert de projeção; as outras nove
contagens existiam como asserções de `len()` separadas, no mesmo estado. Vale
como lembrete de que "não encontrei" e "não existe" são afirmações diferentes.

### Como contar o mecanismo legado

Durante a migração, quatro cardinalidades diferentes foram todas chamadas de
"sítio" em algum momento, e um relatório chegou a publicar um número que não
media nenhuma delas. Os nomes ficam fixados aqui:

| métrica | valor | o que conta |
|---|---:|---|
| `source_locations` | 27 | chamadas de `stable_region_projection` no fonte |
| `behavioral_projection_sites` | 31 | projeções distintas produzidas em execução |
| `measurement_assertion_statements` | 27 | comandos `assert_eq!` que fixam medida |
| `measurement_expectation_tuples` | 31 | pares literais `(length, fnv)` esperados |
| `unique_snapshots` | 13 | estados históricos distintos |

A diferença entre 27 e 31 é inteira: duas das 27 chamadas estão dentro de laços
que iteram sobre três expectativas cada. `25 + 3 + 3 = 31`.

A equivalência com a autoridade nova é medida em
`behavioral_projection_sites` — 31 — porque é o número de projeções que
realmente existem para comparar byte a byte. "Sítio", sozinho, deixou de ser
termo aceitável.

### Cutover concluído

O cutover da autoridade de projeções está feito. Os **31 casos históricos** da
cartografia consultam os 13 snapshots por identificador; nenhum recalcula
projeção estável, comprimento ou FNV. Os únicos valores de `regions`, `length` e
`fnv1a64` vivem nos TOML.

O mecanismo procedural que reconstruía aquelas projeções saiu: 17 helpers
históricos, `stable_region_projection` e o FNV local do harness foram removidos
por não terem mais consumidor.

### O harness estrutural histórico

Sobrou um resíduo *test-only*, e ele não é uma segunda autoridade.

Nove gates das ondas afirmam propriedades estruturais sobre estados históricos
que **nunca tiveram medida congelada** — por exemplo, quantas regiões de
evidência existiam na Onda 8E. Esses estados não são snapshots e não vão virar
snapshots: fabricar uma medida que a história não produziu seria pior que o
problema.

Para eles ficaram três funções de *membresia*:

```
retain_membership_base
historical_membership_onda_8e
historical_membership_pre_onda_8f
```

Elas só removem regiões. Está provado que nenhuma das asserções independentes
desses nove gates observa `hash` ou `summary` — os dois campos que a
reconstrução antiga também restaurava —, então essas restaurações não
sobreviveram ao cutover. O harness não calcula FNV, não calcula comprimento, não
verifica snapshot, não tem medidas e não participa de lifecycle.

A fronteira é verificada, não prometida: uma guarda permanente exige os 31 casos
com a distribuição exata por snapshot, recusa identificador inexistente, e falha
se o harness voltar a conter cálculo de projeção, FNV, ou restauração de `hash`
ou `summary`.

### Por que a distribuição, e não só a contagem

A guarda fixa quantos casos cabem a cada snapshot, não apenas que somam 31.

O motivo é uma regressão real desta migração: uma limpeza de código morto apagou
oito verificações canônicas junto com as cadeias que as cercavam, e a suíte
continuou **inteiramente verde** — nenhum teste falha por existir em menor
número. A contagem total teria sido recomposta por qualquer repetição; a
distribuição não.

<!-- @pinker-doc:end development.projection-snapshots-contract.composicao -->
