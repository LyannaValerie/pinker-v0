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

# Contrato do arquivo histórico das projeções de navegação

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
tags: [desenvolvimento, snapshots, projecoes, determinismo, cartografia]
aliases:
  - snapshot de projecao
  - projecao estavel
  - medidas historicas da cartografia
summary: Projeção estável, medidas históricas preservadas, e a fronteira entre a projeção documental e o arquivo histórico da cartografia de código.
-->
## Domínio e fronteira

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
id: development.projection-snapshots-contract.arquivo
tags: [desenvolvimento, arquivo, integridade, sha256, proveniencia]
aliases:
  - arquivo historico materializado
  - indice do arquivo
  - verificacao de integridade historica
summary: Formato do arquivo materializado em .pinker/archive/, o índice de proveniência, o que a verificação confere e o que ela deliberadamente não lê.
-->
## O arquivo materializado

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

**Proveniência declarada.** Os bytes materializados são a projeção histórica
reconstruída e aceita, exportada no momento do cutover. Não se afirma que eles
tenham sido capturados contemporaneamente à data histórica original. O índice
carrega essa afirmação explicitamente, em `provenance`, para que nenhum leitor
futuro precise inferi-la.

## O que a verificação confere

Para cada entrada, `pink nav projecao verificar` exige:

- o payload existe;
- o comprimento em bytes é o `length` preservado;
- a contagem de registros é o `regions` preservado;
- o FNV-1a64 sobre os bytes é o `fnv1a64` preservado;
- o SHA-256 sobre os bytes é o `sha256` declarado;
- o metadado histórico original existe e ainda tem o `metadata_sha256` que o
  índice registrou;
- `id` e `payload_path` são únicos no índice.

| Resultado | Quando |
|---|---|
| `INTACT` | toda entrada íntegra |
| `ALTERED` | ao menos uma medida ou digest diverge dos bytes observados |
| `MISSING` | um payload ou metadado histórico não existe |

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
<!-- @pinker-doc:end development.projection-snapshots-contract.arquivo -->

<!-- @pinker-doc:start
id: development.projection-snapshots-contract.acervo
tags: [desenvolvimento, historia, identidade, eras, frozen]
aliases:
  - identificadores historicos
  - acervo de treze estados
  - metadados frozen preservados
summary: Os treze estados históricos aceitos, os seus identificadores, as seis eras que os organizam e o papel residual dos TOML FROZEN preservados.
-->
## Os treze estados aceitos

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
<!-- @pinker-doc:end development.projection-snapshots-contract.acervo -->
