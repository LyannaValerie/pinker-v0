<!--
Template de PR da Trama Pinker (especificação §13).

O corpo narrativo abaixo é voltado a HUMANOS. O bloco ```pinker-change``` no
final é voltado à AUTOMAÇÃO e deve permanecer separado da narrativa. Para PRs
posteriores ao marco #330, importe o bloco com:

    pink doc importar-pr <n> --corpo <arquivo>

O CI valida em modo somente leitura (`--check`); ele não sincroniza nem cria
commits. Sincronizar catálogos e projeções é responsabilidade de quem abre o PR.
-->

## Resumo

<!-- O que este PR faz, em linguagem humana. -->

## Motivação

<!-- Por que esta mudança é necessária agora. -->

## Detalhes técnicos

<!-- Como foi implementado; decisões relevantes. -->

## Limites

<!-- O que este PR NÃO faz; limitações honestas; nada declarado sem evidência. -->

## Validação

<!-- Como foi validado. Ex.: make ci, pink doc verificar, pink nav verificar. -->

- [ ] `make ci`
- [ ] `pink doc verificar`
- [ ] `pink nav verificar`

## Bloco estruturado

<!--
Preencha o bloco abaixo apenas para mudanças posteriores ao marco #330.
Mantenha-o separado da narrativa acima. Campos e enums seguem
.pinker/schemas/change-v1.schema.json.
-->

```pinker-change
schema: 1
kind: phase            # phase | hotfix | documentation | parallel-phase
title: <título curto da mudança>
status: completed      # completed | in-progress | planned
area:
  - <territorio.ou.dominio>
updates:
  state: false
  history: false
  roadmap: false
validation:
  required:
    - make ci
```
