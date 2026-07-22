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
- `self_executable_replacement_keeps_typed_pinker_runnable`, em
  `tests/agent_runner_tests.rs`, protege a troca do executável durante a própria
  execução;
- `git_status_first_line_is_preserved_and_rejects_out_of_scope`, em
  `tests/agent_limits_tests.rs`, protege a primeira linha significativa de
  `git status --porcelain` e sua rejeição de escopo.

Uma execução histórica herdou o diretório temporário do sistema e permanece
registrada como `HISTORICAL_REJECTED_RUN`: ela não serve como evidência de
aceitação. A retomada executa novamente dogfood, sensibilidade e validação com
`TMPDIR`, `TMP`, `TEMP` e `CARGO_TARGET_DIR` confinados à raiz delegada; apenas
essa execução superveniente pode demonstrar respeito às limitações.

## Não-alegações

Esta onda não conclui `pink agente`, não conclui a Trama, não ativa `apps/`, não
ativa a Onda 9 e não transforma limites declarativos em isolamento do SO.
<!-- @pinker-doc:end development.pink-agent.contract -->
