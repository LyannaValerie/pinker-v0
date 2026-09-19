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

# Historical archive contract of the navigation projections

- **Classe:** Engine
- **Papel:** contrato de domínio
- **Status:** ativo

Este documento descreve como a Pinker guarda e verifica os estados históricos
da cartografia de código. Ele foi reescrito pela unidade TA (Issue #697), que
aposentou a reconstrução desses estados a partir do catálogo corrente e passou
a guardá-los como bytes materializados.

A mudança é de natureza, não de formato de saída. Antes, provar um estado
histórico exigia que o código de hoje conseguisse reconstruí-lo: cada mudança
legítima do presente — renomear uma chave, reescrever um resumo, editar o corpo
de uma região — quebrava a reconstrução até que alguém ensinasse ao mecanismo
uma nova forma de restauração inversa. O passado cobrava manutenção do presente.
Agora o estado aceito de cada marco está preservado nos seus bytes exatos, e
verificá-lo é conferir integridade desses bytes.

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.schema
tags: [development, snapshots, projections, determinism, cartography]
aliases:
  - projection snapshot
  - stable projection
  - historical cartography measures
summary: Stable projection, preserved historical measures, and the boundary between the document projection and the historical archive of the code cartography.
-->
## Domain and boundary

`src/projection.rs` é a autoridade das projeções **documentais**: ele projeta os
manifestos versionados em regiões geradas de documentos humanos (§12). O arquivo
histórico é **outro domínio**: preserva a projeção estável das regiões do
catálogo de código num momento aceito. A coincidência da palavra "projeção" é
terminológica; os dois módulos não compartilham modelo, formato nem regras.

## Projeção estável

A projeção estável de um conjunto de regiões contém apenas campos que não mudam
com edições irrelevantes: schema, `key`, `kind`, `domain`, `layer`, `file`,
`summary`, `hash` e `status`. Um registro por região, terminado em `\n`,
ordenado lexicograficamente e concatenado. Números de linha ficam de fora de
propósito.

Como `file` já é repo-relativo, a projeção é idêntica em qualquer root absoluto.

## Medidas preservadas

Cada estado histórico preserva exatamente três medidas da sua projeção estável:

| Medida | Significado |
|---|---|
| `regions` | quantidade de registros |
| `length` | comprimento em bytes da projeção |
| `fnv1a64` | FNV-1a 64 da projeção, em `fnv1a64:` + 16 hexadecimais minúsculos |

Estas três são as medidas **históricas**. Elas não são recalibradas, nem
substituídas, nem derivadas de novo. O SHA-256 que o índice do arquivo declara é
evidência **adicional** de integridade do arquivo, e não participa da identidade
histórica de nenhum estado.
<!-- @pinker-doc:end development.projection-snapshots-contract.schema -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.archive
tags: [development, archive, integrity, sha256, provenance]
aliases:
  - materialized historical archive
  - archive index
  - historical integrity verification
summary: Format of the archive materialized under .pinker/archive/, the provenance index, what verification checks and what it deliberately does not read.
-->
## The materialized archive

O acervo vive em `.pinker/archive/`:

| Caminho | Conteúdo |
|---|---|
| `.pinker/archive/<id>.stable` | os bytes exatos da projeção estável aceita |
| `.pinker/archive/index.toml` | o índice de proveniência e integridade |

O diretório é separado de `.pinker/projections/` de propósito. Aquele diretório
é uma autoridade enumerada estritamente, que recusa qualquer entrada que não
seja um TOML de snapshot; um subdiretório ali faria a autoridade corrente
reportar artefato inválido.

## Índice de proveniência

```toml
schema = 1
export_source_main = "<commit de main no momento da exportação>"
export_source_tree = "<árvore desse commit>"
export_method = "<como os bytes foram obtidos>"
provenance = "<o que os bytes são e o que não são>"

[[entries]]
id = "identificador-estavel"
metadata_path = ".pinker/projections/identificador-estavel.toml"
metadata_sha256 = "<64 hexadecimais minúsculos>"
payload_path = ".pinker/archive/identificador-estavel.stable"
regions = 416
length = 192709
fnv1a64 = "fnv1a64:6dd6fef55bf7eeac"
sha256 = "<64 hexadecimais minúsculos>"
```

O parser é estrito. Rejeita schema ausente ou desconhecido, seção nomeada,
array de tabelas desconhecido, chave desconhecida, chave duplicada, campo
obrigatório ausente, texto onde se espera inteiro e vice-versa, número negativo,
overflow, string incompleta, escape não suportado, dado residual após o valor,
medida fora da forma canônica, índice sem nenhuma entrada, `id` repetido e
caminho de payload repetido.

**Caminhos canônicos e confinados.** `payload_path` e `metadata_path` não são
strings livres. Cada um passa primeiro pela política lexical do núcleo de
automação (`RelativePath`), que recusa caminho vazio, absoluto, com `..`, com
componente degenerado, com barra invertida, com caractere de controle ou longo
demais — e portanto nenhum caminho que escapa chega a virar um `root.join`.
Depois, cada um tem de ser exatamente o caminho canônico da sua própria
entrada: `.pinker/archive/<id>.stable` e `.pinker/projections/<id>.toml`. Um
caminho lexicamente inocente mas apontando para outro diretório, ou para a
identidade de outro estado, não descreve aquela entrada e é recusado.

**Proveniência declarada.** Os bytes materializados são a projeção histórica
reconstruída e aceita, exportada no momento do cutover. Não se afirma que eles
tenham sido capturados contemporaneamente à data histórica original. O índice
carrega essa afirmação explicitamente, em `provenance`, para que nenhum leitor
futuro precise inferi-la.

## Qual é a autoridade das medidas

O índice **não** é a autoridade da história. Ele é uma cópia escrita no
cutover, e uma cópia nunca é a autoridade daquilo que copia. A autoridade das
medidas históricas é o `[measures]` de cada TOML FROZEN preservado em
`.pinker/projections/`, e a autoridade do *conjunto* aceito é o próprio
diretório: um TOML por estado, nem mais nem menos.

Isso decide a ordem em que `pink nav projecao verificar` trabalha:

```text
enumerar os metadados históricos preservados
-> exigir correspondência um-para-um entre eles e as entradas do índice
-> validar os caminhos canônicos confinados
-> ler as medidas preservadas no [measures] de cada TOML FROZEN
-> exigir que as medidas do índice sejam exatamente essas
-> medir o payload contra as medidas preservadas
-> exigir o SHA-256 do payload declarado no índice
-> exigir o SHA-256 do metadado declarado no índice
-> INTACT
```

Conferir o payload apenas contra o índice fecharia sozinho: bastaria editar os
bytes e, em seguida, editar o índice para concordar com eles, e a história
estaria recalibrada em silêncio. É por isso que a ancoragem no `[measures]`
congelado é verificação de produto, não asserção de suíte.

E iterar apenas `index.entries` teria o defeito simétrico: quem percorre o que
o índice declara não tem como notar o que ele deixou de declarar. Retirar uma
entrada junto com o seu payload deixaria as restantes íntegras. A cobertura
um-para-um contra os metadados preservados é o que fecha essa porta — um estado
aceito sem entrada falha, uma entrada que não nomeia estado aceito algum falha,
e um arquivo que não seja um TOML preservado nessa autoridade também falha,
para que retirar um estado da história não seja apenas renomear uma extensão.

## O que a verificação confere

Para cada entrada, `pink nav projecao verificar` exige:

- o metadado histórico original existe e ainda tem o `metadata_sha256` que o
  índice registrou;
- o `[measures]` desse metadado declara `regions`, `length` e `fnv1a64`;
- `regions`, `length` e `fnv1a64` do índice são exatamente esses literais;
- o payload existe;
- o comprimento em bytes é o `length` preservado;
- a contagem de registros é o `regions` preservado;
- o FNV-1a64 sobre os bytes é o `fnv1a64` preservado;
- o SHA-256 sobre os bytes é o `sha256` declarado;
- `id` e `payload_path` são únicos no índice, e os caminhos são canônicos.

| Resultado | Quando |
|---|---|
| `INTACT` | toda entrada íntegra |
| `ALTERED` | ao menos uma medida ou digest diverge da autoridade preservada |
| `MISSING` | um payload ou metadado histórico não existe |

As divergências de ancoragem aparecem nomeadas: `frozen_regions`,
`frozen_length` e `frozen_fnv1a64` dizem que o índice deixou de repetir o TOML
congelado, e `frozen_measures` diz que o próprio `[measures]` não pôde ser
lido. A cobertura e o confinamento falham antes disso, ao carregar o índice,
porque um acervo incompleto ou com caminho fora da autoridade não descreve um
arquivo do qual se possa concluir coisa alguma.

## O que a verificação não lê

Esta é a propriedade que a unidade TA entregou, e ela é verificada por prova
comportamental — um repositório que contém apenas o marcador de raiz, o arquivo
e os metadados FROZEN verifica `INTACT`:

- `src/navigation.jsonl`;
- chaves, resumos, hashes, paths ou domínios correntes;
- mapa de renomeação;
- receitas de reconstrução;
- qualquer regra de restauração inversa.

Portanto:

```text
CURRENT_CATALOG_DRIFT != HISTORICAL_ARCHIVE_DRIFT
LEGITIMATE_CURRENT_CHANGE -> ZERO_HISTORICAL_MAINTENANCE
```

Um arquivo histórico não fica "defasado" em relação ao catálogo corrente, e o
estado consolidado do projeto não o descreve assim: ele reporta integridade, não
sincronismo.

## Superfície pública

`pink nav projecao` é somente leitura, com três comandos:

| Comando | Papel |
|---|---|
| `pink nav projecao listar` | inventário do acervo e da sua origem |
| `pink nav projecao mostrar ID` | uma entrada, com medidas preservadas e integridade observada |
| `pink nav projecao verificar [ID]` | integridade do acervo inteiro ou de uma entrada |

Os códigos de saída distinguem `0` íntegro, `3` payload ou índice ausente, `4`
identificador desconhecido, `5` arquivo alterado e `6` índice inválido.

`preparar`, `aceitar` e `reconciliar` foram retirados com o mecanismo que
serviam. Preparar um candidato, aceitá-lo e reconciliar receitas eram operações
de reconstrução; sem reconstrução, elas não têm o que fazer.

`--observado` foi retirado junto. A opção pedia a observação reconstruída do
estado histórico a partir do catálogo corrente, e essa observação deixou de
existir. O que ela tinha de útil — confrontar o estado guardado com as medidas
preservadas — `mostrar ID` passou a fazer sempre, sobre o payload
materializado, e reporta o resultado em `integridade`. `--justificativa`,
`--predecessor`, `--autorizar` e `--renomeacoes` eram opções do lifecycle de
reconstrução e saíram com ele.
<!-- @pinker-doc:end development.projection-snapshots-contract.archive -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.collection
tags: [development, history, identity, eras, frozen]
aliases:
  - historical identifiers
  - collection of thirteen states
  - preserved frozen metadata
summary: The thirteen accepted historical states, their identifiers, the six eras that organize them and the residual role of the preserved FROZEN TOML files.
-->
## The thirteen accepted states

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

Os 13 estados se organizam em **6 eras temporais**. A primeira era concentra
oito deles: são recortes de escopo do mesmo momento histórico, medidos por gates
diferentes, cada um excluindo as regiões da própria evidência. Eras não reduzem
a contagem de estados — um recorte com medida própria é um estado próprio.

## Os TOML FROZEN preservados

Os 13 arquivos `.pinker/projections/<id>.toml` continuam no repositório, byte a
byte como sempre estiveram. Eles deixaram de ser executados: ninguém aplica mais
as suas regras de reconstrução. O papel deles agora é **proveniência** — o
registro histórico de como aquele estado era descrito quando ainda era
reconstruído — e o índice do arquivo prende os bytes de cada um por SHA-256, de
modo que editá-los é detectado.

Isso não é uma segunda autoridade concorrente. A autoridade do fato histórico é
o payload materializado e as suas medidas preservadas; o TOML é evidência de
origem, e é verificado como tal.

## Proveniência das medidas

As três medidas de todos os 13 estados são **literais históricos migrados**:
`regions`, `length` e `fnv1a64` já existiam no legado e atravessaram a
materialização sem recálculo. O que a unidade TA acrescentou foi o SHA-256, que
é integridade do arquivo, e não medida histórica.

Antes de exportar qualquer byte, a equivalência foi provada nos dois sentidos
enquanto os dois mecanismos coexistiam: cada payload foi comparado byte a byte
com `stable_projection()` da reconstrução aceita, e a exportação recusava
qualquer estado cuja reconstrução não reproduzisse as próprias medidas
preservadas. Só depois de 13/13 equivalentes, com o checkpoint verde no CI
remoto, a reconstrução foi retirada.
<!-- @pinker-doc:end development.projection-snapshots-contract.collection -->
