<!--
Template de PR da Trama Pinker (especificação §13).

O corpo narrativo abaixo é voltado a HUMANOS. O bloco ```pinker-change``` no
final é voltado à AUTOMAÇÃO e deve permanecer separado da narrativa. Para PRs
posteriores ao marco #330, importe o bloco com:

    ./ci_env.sh cargo run --bin pink -- doc importar-pr <n> --corpo <arquivo>

O CI valida em modo somente leitura (`--check`); ele não sincroniza nem cria
commits. Sincronizar catálogos e projeções é responsabilidade de quem abre o PR.
-->

## Resumo

<!-- O que este PR faz e por que a mudança é necessária, em linguagem humana. -->

## Detalhes e limites

<!-- Como foi implementado, decisões relevantes, o que NÃO faz e limitações honestas. Para uma correção pequena, seja breve. -->

## Validação

<!-- Consulte CONTRIBUTING.md e CODE_OF_CONDUCT.md. Marque somente comandos realmente executados; deixe os demais desmarcados e explique limitações. O CI executará make ci. -->

- [ ] `make ci`
- [ ] `./ci_env.sh cargo run --bin pink -- doc verificar`
- [ ] `./ci_env.sh cargo run --bin pink -- nav verificar`

## Bloco estruturado

<!--
Preencha o bloco abaixo apenas para mudanças posteriores ao marco #330.
Mantenha-o separado da narrativa acima. Campos e enums seguem
.pinker/schemas/change-v1.schema.json.

Propósito: fornecer à automação metadados verificáveis sem tentar interpretar o
resumo humano. Preencha `kind`, `title`, `status` e `area`; use `updates` somente
para famílias de projeção configuradas em `.pinker/doc.toml` e marque `true`
apenas quando a mudança declarar atualização daquela família. Não derive flags
de nomes de arquivos comuns, como `README.md`. Se a classificação não estiver
clara, peça orientação à manutenção antes de inventar uma fase, hotfix ou rodada
documental.

NÃO deixe comentários dentro do bloco ```pinker-change```: ele é lido pela
automação, não pelo YAML padrão. Substitua TODAS as sentinelas <preencher-...>
por valores reais antes de abrir ou atualizar o PR — sentinelas remanescentes
falham no CI com E-CHANGE-PLACEHOLDER.

Valores aceitos:
  kind:   phase | hotfix | documentation | parallel-phase
  status: completed | in-progress | planned
  area:   ids semânticos de território/domínio, ex.: development.trama,
          language.result (formato [a-z0-9]+([._-][a-z0-9]+)*)
-->

```pinker-change
schema: 1
kind: <preencher-kind>
title: <preencher-titulo>
status: <preencher-status>
area:
  - <preencher-area>
updates:
  state: false
  history: false
  roadmap: false
validation:
  required:
    - make ci
```
