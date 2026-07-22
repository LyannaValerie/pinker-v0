---
pinker-doc: 1
id: development.pink-agent
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
related:
  - development.pink-agent-roadmap
  - development.code-navigation
---

# `pink agente` — runner operacional local

<!-- @pinker-doc:start
id: development.pink-agent.contract
tags: [agente, runner, spec, artefatos, limitacoes]
aliases:
  - pink agente
  - agent spec v1
summary: Contrato V1-A do runner local auditável para tarefas operacionais Pinker.
-->
## Problema e superfície

Prompts operacionais repetem captura de workspace, comandos Pinker-first, build,
testes, inspeção Git, integridade e resultado. A Onda A mecaniza o núcleo local:

```text
pink agente iniciar <spec>
pink agente executar <spec>
pink agente verificar <spec>
pink agente sensibilidade <spec>
pink agente publicar <spec>
pink agente retomar <spec>
pink agente status <spec> [--json]
pink agente relatorio <spec>
```

## Agent-spec-v1

O formato é line-oriented, `chave = valor`, determinístico e sem dependências.
`schema = 1` identifica a versão. Escalares não podem repetir; listas usam as
chaves repetíveis `allowed_write`, `allowed_change` e `command.<id>.arg`.
Campos desconhecidos falham. Cada comando declara `kind`, `program`, `cwd`,
`expect` e `shell`; ambientes autorizados usam `command.<id>.env.NOME`. `kind =
pinker` exige `program = pink`. Interpretadores de shell são recusados quando
`shell = false`.

Raízes absolutas `repo_root`, `worktree` e `delegated_root` são confinadas ao
repositório. CWDs e mudanças são relativos ao worktree; escritas declaradas são
relativas ao diretório delegado. Componentes `..` são rejeitados.

## Estado e artefatos

`iniciar` registra ambiente, workspace e estado `READY`. `executar` captura
stdout, stderr, duração, shell e exit code; persiste logs por ID, emite eventos
JSONL monotônicos e marca etapas posteriores como `NOT_RUN` após falha. Ao fim,
gera snapshots, escopo, validação, `resultado.json`, `RELATORIO.md` e manifesto
SHA-256 ordenado, sem auto-hash. JSON e Markdown canônicos usam substituição
atômica.

Estados terminais são `ACCEPTED`, `BLOCKED` e, como contrato reservado,
`NEEDS_HUMAN_DECISION`. Os códigos são respectivamente 0, 1 e 2. Erro de
harness bloqueia a execução.

## Diretório delegado e limitações

O spec aponta o diretório delegado. O runner só cria `artefatos/`, `estado/` e
`logs/` nele; comandos recebem CWD confinado e apenas variáveis explicitamente
declaradas, além do ambiente mínimo de execução. O V1-A não publica PR, não
gerencia checks remotos, não faz merge, não implementa retomada e não oferece
sandbox de sistema operacional: a fronteira é validação mecânica mais processo
estruturado.

## Dogfood da Onda A

A tarefa real usa o runner para workspace, Pinker-first, baseline, testes-alvo,
sincronização do catálogo, validação e escopo. A cartografia marker-only cobre
`trama_ci_tests` e `trama_template_tests`. O dogfood encontrou e corrigiu duas
regressões do núcleo: exigência de formatação antes da primeira execução e
normalização incorreta da primeira linha modificada no status Git.

### Achados operacionais observados

- `pink nav` e `pink doc` apresentaram interfaces claras depois que o binário
  ficou disponível; antes disso, `target/debug/pink` não existia e foi preciso
  compilar e usar o executável no `CARGO_TARGET_DIR` delegado;
- `pink doc rota` teve pouco valor antes da sincronização documental e depois
  encontrou `development.pink-agent.contract`;
- o maior atrito veio dos contratos do repositório: `target/` local exigido
  pela fase220, catálogos sincronizados, códigos de saída de drift e localização
  do binário delegado;
- `pink doc importar-pr --check` rejeitou corretamente `kind: feature` com
  `E-CHANGE-SCHEMA`; a publicação usa o tipo canônico `parallel-phase`;
- o teste remoto `self_executable_replacement_keeps_typed_pinker_runnable`
  falhou no GitHub Actions com `ETXTBSY` (`ExecutableFileBusy` / `Text file
  busy`): ele sobrescrevia o executável ativo e, portanto, confundia o contrato
  de produção com uma mutação de filesystem não portável;
- `resolve_pinker_executable` preserva o caminho corrente quando ele existe e,
  quando recebe um caminho inexistente terminado pelo sufixo literal `
  (deleted)`, remove esse sufixo uma vez e exige que o substituto exista;
- as regressões puras `resolve_existing_current_executable`,
  `resolve_deleted_current_executable_to_replacement` e
  `reject_deleted_current_executable_without_replacement` cobrem a resolução
  sem modificar o executável ativo; a execução real sem shell implícito segue
  coberta por `comando_pinker_tipado_executa_pela_cli`;
- `git_status_first_line_is_preserved_and_rejects_out_of_scope`, em
  `tests/agent_limits_tests.rs`, protege a primeira linha significativa de
  `git status --porcelain` e sua rejeição de escopo.

Uma execução histórica herdou o diretório temporário do sistema e permanece
registrada como `HISTORICAL_REJECTED_RUN`: ela não serve como evidência de
aceitação. A retomada executa novamente dogfood, sensibilidade e validação com
`TMPDIR`, `TMP`, `TEMP` e `CARGO_TARGET_DIR` confinados à raiz delegada; apenas
essa execução superveniente pode demonstrar respeito às limitações.

## Onda B completa

A Onda A completa entregou o núcleo V1-A. A Onda B completa preserva o schema
1 e acrescenta `sensibilidade`, checks Git/diff, marker-only e projection. O
check Git compara HEAD, branch, contagem de commits, porcelain ordenado e
`git diff --check`; marker-only reconstrói os bytes sem linhas Pinker reais; e
projection mede a visão estável do catálogo com exclusões e overrides
explícitos. O mutation runner aplica snippets atomicamente, executa probes sem
shell implícito, captura stdout/stderr e restaura os bytes e o SHA-256.

O dogfood usa essas quatro capacidades sobre a própria mudança e cartografa
`trama_manifest_tests.rs` e `trama_sync_tests.rs`.

## Onda C completa

A Onda C completa preserva o schema 1 e a compatibilidade V1-A/V1-B. O check
`pr-body` confina e valida o corpo local, exige um único bloco
`pinker-change`, compara kind, title, area e validation e persiste SHA-256 e
a execução canônica. `publicar` valida o estado local, faz staging por paths
exatos, cria um commit, um push normal e uma única PR; `retomar` reconcilia
spec, commit, branch, PR, body, candidato e checks sem repetir ações comprovadas.

A máquina de estados persiste `LOCAL_ACCEPTED`, intenções antes de commit,
push e PR, resultados reconciliados, `BODY_VERIFIED`, `CHECKS_PENDING`,
`ACCEPTED`, `BLOCKED` ou `NEEDS_HUMAN_DECISION`. A allowlist GH limita a
execução a equivalentes de `pr list`, `pr create`, `pr view` e `pr
checks`; não há edição remota, rerun, merge nem auto-merge. No dogfood real,
`publicar` para em `CHECKS_PENDING` e `retomar` aguarda os checks exatos
do SHA candidato.

Como `ci.yml` dispara em `push` e `pull_request`, o mesmo SHA pode expor
múltiplas ocorrências de um check requerido — várias linhas `rust`, por
exemplo. A multiplicidade, sozinha, nunca bloqueia: `retomar` agrega todas as
ocorrências de cada nome requerido pela função pura
`classify_required_check_states` e aplica a precedência `BLOCKED > PENDING >
SUCCESS`. Zero ocorrências conta como pendente, qualquer estado desconhecido
bloqueia e checks extras não substituem um requerido ausente. A identidade
exata do candidato continua obrigatória. A duplicidade permanece inválida
apenas quando declarativa — a mesma linha `required_check` repetida na spec é
rejeitada no parsing. O run histórico rejeitado por
`LOCAL_CHECK_AGGREGATION_BUG` é preservado sem rerun nem bypass; a Onda D
reexercita o fluxo corrigido em estado limpo.

Com `trama_projection_tests.rs` e `trama_scale_tests.rs`, as seis suítes
operacionais previstas nas Ondas A–C estão cartografadas. A Trama ainda não tem
fechamento formal: `trama_complete = false`; a Onda D é próxima e separada, a
Onda 9 inativa e `apps/` reservada.

## Não-alegações

Não há sandbox de SO. Marker-only não prova qualidade semântica; projection não
prova significado de negócio; sensibilidade não é cobertura exaustiva; e a
restauração não é uma transação crash-proof nem atomicidade distribuída.
Disponibilidade do GitHub não é garantida; divergências remotas não são
corrigidas automaticamente e checks não recebem rerun. A Onda C não conclui a
Trama, não ativa a Onda 9 e mantém `apps/` reservada.
<!-- @pinker-doc:end development.pink-agent.contract -->
