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

Este documento descreve o contrato **somente leitura** dos snapshots históricos
das projeções do catálogo de navegação, implementado em
`src/nav_projection_snapshot.rs` sob a segunda capacidade da janela auxiliar
(`janela-infraestrutura-deterministica.md`, Issue #384).

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

Formato TOML, um arquivo por snapshot, em `.pinker/projections/<id>.toml`:

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

Só existem dois. `FROZEN` é imutável e nunca é atualizado implicitamente.
`CANDIDATE` existe no modelo, mas este recorte não prepara nem aceita candidatos:
o ciclo de vida mutável e a superfície de CLI pertencem a etapas posteriores.
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

## Fora deste recorte

- criação de snapshots reais e migração das medidas históricas;
- preparação e aceitação de candidatos;
- escrita em disco, descoberta de root e temporários;
- superfície de CLI;
- qualquer alteração do contrato congelado `pink-agent-v1`.
<!-- @pinker-doc:end development.projection-snapshots-contract.reconstrucao -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.composicao
tags: [desenvolvimento, snapshots, composicao, receitas, schema]
aliases:
  - schema 2 de snapshot
  - receita de reconstrucao
  - composicao de reconstrucao
summary: Schema 2 do snapshot, formato de receita e os invariantes do grafo de composição.
-->
## Quantas receitas a migração vai criar

Não se sabe ainda, e o número **não** é derivável da contagem de helpers.

O inventário encontrou 15 helpers de reconstrução. Oito são nós puramente
intermediários e sete são pontos de entrada compartilhados entre snapshots —
todos os quinze são candidatos a receita, porque todos são reutilizados.

O número final sai da **semântica**, não da contagem. Um helper só pode ser
colapsado quando as três condições valerem ao mesmo tempo:

- é mera delegação, sem regras locais próprias;
- não marca fronteira procedural relevante, isto é, colapsá-lo não muda a ordem
  observável de aplicação;
- a equivalência exata é demonstrada, não presumida.

Qualquer estimativa anterior a essa análise — inclusive "oito" — é chute.

## Por que existe composição

O inventário refeito sobre a `main` mostrou 15 helpers de reconstrução dispostos
num DAG e apenas 13 estados com medida histórica própria. **Oito helpers são nós
puramente intermediários**: nenhum teste os chama e nenhum produz medida.

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
| versão atual | `schema = 2` | `schema = 1` |

Os formatos são versionados de forma independente: o `schema = 2` pertence ao
snapshot, que foi quem ganhou composição. A receita nasce agora e estreia em 1.

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
| snapshot com versão fora de {1, 2} | `E-SNAP-SCHEMA` | "schema N desconhecido para snapshot; este formato aceita 1 ou 2" |
| receita com versão fora de {1} | `E-RECEITA-SCHEMA` | "schema N desconhecido para receita; este formato aceita somente 1" |

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
<!-- @pinker-doc:end development.projection-snapshots-contract.composicao -->
