---
pinker-doc: 1
id: development.consolidated-project-state-contract
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.consolidated-project-state-contract
related:
  - development.automation-core-contract
  - development.projection-snapshots-contract
  - development.deterministic-infrastructure-window
---

# Consolidated project state contract

- **Classe:** Engine
- **Papel:** contrato observacional
- **Status:** ativo

Este documento define o recorte somente leitura da Issue #387, autorizado pela
janela da Issue #417. A superfície reúne fatos já possuídos pela Trama, pela
documentação, pelo arquivo histórico e pelo automation core; ela não cria
uma segunda fonte de verdade para nenhum deles.

<!-- @pinker-doc:start
id: development.consolidated-project-state-contract.model
tags: [development, state, schema, determinism, domains]
aliases:
  - consolidated state
  - project state
  - project state schema
summary: Versioned model, domains, states, overall and source attribution of the read-only consolidated query.
-->
## Model and schema

O modelo Rust reutilizável é `ProjectState`, produzido por adapters somente
leitura e consumido diretamente pelos renderers humano e JSON. A versão pública
inicial é:

```text
PROJECT_STATE_SCHEMA = 1
```

Essa versão pertence exclusivamente à consulta consolidada. Ela não reutiliza
`SNAPSHOT_SCHEMA`, `SNAPSHOT_REPORT_SCHEMA`, `PROJECTION_CLI_SCHEMA` nem
`AUTOMATION_SCHEMA`.

Todo relatório contém, no mínimo, `schema`, `overall`, `domains`, `warnings`,
`blockers` e `pending_operations`. Os domínios têm identidade e ordem estáveis:

1. `repository`;
2. `trama`;
3. `documentation`;
4. `projections`;
5. `local_checks`;
6. `diagnostics`.

Os estados JSON canônicos são `OK`, `WARNING`, `BLOCKED`, `UNKNOWN`,
`UNAVAILABLE` e `PARTIAL`. Os três últimos não equivalem a sucesso. O
`overall` é derivado dos domínios, sem depender da ordem de descoberta, pela
precedência `BLOCKED`, `WARNING`, `PARTIAL`, `OK`; um domínio `UNKNOWN` ou
`UNAVAILABLE` contribui para `PARTIAL` quando não há estado mais severo.

Cada domínio e cada fato material identifica uma `source` tipada. Os kinds
iniciais são `repo_file`, `derived`, `local_check`, `catalog` e
`historical_archive`; `authority` carrega um identificador interno estável e
`path`, quando existe, é sempre repo-relativo. Root absoluto, mtime, usuário,
hostname, PID e horário corrente não fazem parte do protocolo.

Disponibilidade parcial é parte do modelo. Uma autoridade inválida bloqueia o
domínio correspondente sem apagar os demais. Índice de arquivo histórico
ilegível é `BLOCKED`, nunca `UNKNOWN`.
<!-- @pinker-doc:end development.consolidated-project-state-contract.model -->

<!-- @pinker-doc:start
id: development.consolidated-project-state-contract.authorities
tags: [development, trama, documentation, projections]
aliases:
  - consolidated state authorities
  - pink estado domains
summary: Adaptation without duplication of the root, Trama, documentation, projections and checks authorities.
-->
## Adapted authorities

`repository` reutiliza `automation::RepoRoot` e publica somente o marcador
repo-relativo e a disponibilidade das autoridades mínimas. Não existe um novo
detector de raiz.

`trama` reutiliza `CodeIndex`, `CodeCatalog` e o mesmo verificador usado por
`pink nav verificar`. `documentation` reutiliza `DocIndex`, `DocCatalog`,
manifestos e projeções documentais do mesmo verificador usado por
`pink doc verificar`. A consulta não executa essas CLIs como subprocessos e não
interpreta sua saída.

`projections` reutiliza o arquivo histórico materializado e a sua verificação de
integridade. O inventário publica quantos estados estão arquivados; os outcomes
preservam `INTACT`, `ALTERED` e `MISSING`, por entrada. O estado consolidado
nunca descreve o arquivo histórico como precisando sincronizar com o catálogo
corrente: um arquivo materializado é íntegro ou comprometido, nunca defasado.

`local_checks` lista somente verificações locais já possuídas pelas autoridades
acima. Ele não introduz `doctor` nem probes ambientais. `diagnostics` agrega os
diagnósticos produzidos pelos adapters; não é um subsistema independente.

<!-- @pinker-doc:end development.consolidated-project-state-contract.authorities -->

<!-- @pinker-doc:start
id: development.consolidated-project-state-contract.surface
tags: [development, cli, json, read-only, exits]
aliases:
  - pink estado
  - pink estado json
summary: CLI, deterministic JSON, warnings, blockers, pending operations, absolute read-only operation and exit codes of the consolidated state.
-->
## Surface and effects

As formas públicas são:

```text
pink estado
pink estado --json
pink estado --repo DIRETÓRIO
```

Ajuda está disponível por `pink help estado`, `pink estado --help` e
`pink estado -h`. Flags desconhecidas, valores ausentes, duplicatas ambíguas e
posicionais inesperados são uso inválido.

Human e JSON são renderizados exatamente do mesmo `ProjectState`; o renderer
não consulta autoridades nem recalcula `overall`. O JSON é UTF-8, uma única
linha, sem ANSI e determinístico byte a byte, com ordem fixa, paths
repo-relativos e sem timestamps incidentais. Nenhuma linha humana acompanha o
documento JSON.

Warnings e blockers carregam `id`, `domain`, `summary`, `source` e `reason`. O
identificador e o reason são estáveis e não são derivados da mensagem humana.
Operações pendentes têm também `kind` e só aparecem quando uma autoridade
declara um fato suficiente, como um catálogo corrente com drift. A integridade
do arquivo histórico não gera operação pendente: um arquivo comprometido é
blocker, não trabalho pendente de sincronização.

A coleta é somente leitura por construção: não grava no repositório ou no
estado do agente, não sincroniza catálogos, não muta o arquivo histórico, não
executa Git remoto, `gh`, `curl` ou qualquer rede e não altera mtime de
autoridades. O automation core fornece `RepoRoot` e convenções determinísticas;
não se cria plan, desired state ou apply artificial para uma consulta.

Códigos de saída:

| Código | Significado |
|---:|---|
| `0` | relatório estrutural produzido, inclusive `WARNING`, `BLOCKED` ou `PARTIAL` |
| `1` | falha interna impediu produzir um relatório estrutural válido |
| `2` | uso inválido |
| `3` | root/autoridade mínima não pôde ser estabelecida antes de qualquer modelo útil |

Drift e harness pertencem ao dado do projeto e não mudam o sucesso da consulta.

Uma futura TUI da Issue #388 deverá consumir `ProjectState` diretamente; não
deverá executar a CLI nem parsear stdout. Este contrato não implementa TUI,
`pink env`, `pink doctor`, `pink comandos`, `pink listar` nem
`pink nav localizar`.
<!-- @pinker-doc:end development.consolidated-project-state-contract.surface -->
