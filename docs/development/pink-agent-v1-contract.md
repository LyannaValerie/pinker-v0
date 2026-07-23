---
pinker-doc: 1
id: development.pink-agent-v1-contract
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
related:
  - development.pink-agent
  - development.pink-agent-roadmap
---

# Contrato congelado `pink-agent-v1`

<!-- @pinker-doc:start
id: development.pink-agent-v1-contract.surface
tags: [agente, contrato, congelado, v1]
aliases:
  - contrato pink agente v1
summary: Superfície canônica e imutável do contrato pink-agent-v1 emitido como artefato byte-estável.
-->
## Superfície canônica

A Onda D congela o contrato `pink-agent-v1` (`contract_version = 1`,
`spec_schema = 1`). O agente serializa esse contrato como um artefato
`artefatos/contract-v1.json` **byte-estável e determinístico**, emitido em toda
execução de `iniciar`. O dígito SHA-256 do artefato é fixo:

```text
contract_id: pink-agent-v1
contract_version: 1
spec_schema: 1
contract_digest: c0115ffd65820e0e0afd04ec7d1642db3fdb0e0bf93b8186540d5cb7ade798f4
```

### Subcomandos (8, ordenados)

`iniciar`, `executar`, `verificar`, `sensibilidade`, `publicar`, `retomar`,
`status`, `relatorio`.

### Tipos de check (4)

`git`, `marker-only`, `projection`, `pr-body`.

### Estados terminais (3)

`ACCEPTED`, `BLOCKED`, `NEEDS_HUMAN_DECISION`.

### Estados de publicação (12)

`LOCAL_ACCEPTED`, `COMMIT_INTENT`, `COMMITTED`, `PUSH_INTENT`, `PUSHED`,
`PR_INTENT`, `PR_CREATED`, `BODY_VERIFIED`, `CHECKS_PENDING`, `ACCEPTED`,
`BLOCKED`, `NEEDS_HUMAN_DECISION`.

### Códigos de saída

`ACCEPTED = 0`, `BLOCKED = 1`, `NEEDS_HUMAN_DECISION = 2`.

### Proibições (todas `false`)

`merge`, `auto_merge`, `force_push`, `workflow_rerun`, `remote_body_edit`.

`status --json`, `resultado.json` e `RELATORIO.md` referenciam o mesmo
`contract_id`, `contract_version` e `contract_digest`.
<!-- @pinker-doc:end development.pink-agent-v1-contract.surface -->

<!-- @pinker-doc:start
id: development.pink-agent-v1-contract.body
tags: [agente, contrato, pr-body, corpo-humano]
aliases:
  - corpo humano pr pink agente
summary: Requisito de corpo humano substantivo das PRs publicadas pelo agente sob o contrato V1.
-->
## Corpo humano substantivo de PR

Toda spec com check `pr-body` exige, nesta ordem exata, seis seções `H2`:
`Resumo`, `Problema`, `Implementação`, `Validação`, `Limitações`,
`Próximo passo`. O `Resumo` é o primeiro conteúdo não vazio; cada seção ocorre
uma única vez; mínimo de 40 caracteres substantivos por seção e 400 no conjunto.
Bloco `pinker-change`, fenced code, comentários HTML, checkbox, marcador,
heading e pontuação isolada não contam como substância humana. Placeholders
(`TODO`, `TBD`, `N/A`, `<preencher…>`) bloqueiam. Exatamente um bloco
`pinker-change` fecha o corpo; após ele só whitespace.

O corpo local e o corpo remoto real são validados diretamente pela **mesma**
função `analyze_human_body`; a validação remota **não** é uma releitura do
local. Os artefatos local e remoto são distintos e não se sobrescrevem:
`pr-body-local.json`, `pr-body-human.json`, `pr-body-remote.md`,
`pr-body-remote.json`. O SHA-256 do corpo inclui tudo; o SHA-256 humano exclui o
bloco. `pink doc importar-pr --check` permanece obrigatório. A agregação de
checks repetidos com o mesmo nome segue `BLOCKED > PENDING > SUCCESS`.
<!-- @pinker-doc:end development.pink-agent-v1-contract.body -->
