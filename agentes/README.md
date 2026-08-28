# Execuções de agentes

Esta raiz é o **root operacional das Tasks da Forja**. Cada Task recebe um
subdiretório físico próprio — um *slot* — que contém tudo que a execução dela
produz: worktree, target do Cargo, caches, temporários, rascunho, logs,
memória, estado e artefatos.

```text
ONE_TASK -> ONE_OBSERVED_TASK_ROOT
ALL_DISPOSABLE_TASK_RESOURCES ⊂ TASK_ROOT
```

Somente esta política e o `.gitignore` são versionados; material de execução
nunca entra na PR.

## O slot não é o TASK_ID

O nome do diretório é alocado pela Forja e **não** deriva da identidade da
Task:

```text
TASK_ID   = identidade lógica estável   (issue-536-forja-agentes-restructure)
TASK_SLOT = root físico observado        (a01)
```

Isso mantém o caminho físico curto sem encurtar a identidade — a pressão de
`SUN_LEN` nos sockets unix dos testes nativos deixa de ser paga com o nome da
Task. Nenhum consumidor pode reconstruir o root por concatenação; o vínculo
mora em `<slot>/task.json` e é **observado**:

```bash
forja-agentes observe --json
```

## Layout de um slot

```text
agentes/<slot>/
├── task.json     vínculo TASK_ID <-> slot (autoridade do observador)
├── worktree/     git worktree registrada da Task        [DURABLE]
├── target/       CARGO_TARGET_DIR                       [EPHEMERAL]
├── cache/        cache local da Task                    [EPHEMERAL]
├── tmp/          TMPDIR                                 [EPHEMERAL]
├── scratch/      rascunho descartável                   [EPHEMERAL]
├── logs/         logs de execução                       [EPHEMERAL]
├── memory/       memória factual JSON/JSONL             [DURABLE]
├── state/        checkpoint e retomada                  [DURABLE]
└── artifacts/    evidência e artefatos                  [DURABLE]
```

`EPHEMERAL` é o que o `EXECUTION_SEAL` pode recuperar assim que a PR está de
pé. `DURABLE` só desaparece no `TASK_RETIRE`, depois do merge humano — a
revisão adversarial e a correção ainda precisam da worktree e da memória.

A autoridade é única: a mesma tabela de recursos provisiona, observa, sela e
retira, de modo que `PROVISIONER_SET == FINALIZER_SET` seja propriedade
estrutural e não promessa.

Fonte: `scripts/forja/forja_agentes.py`. Contrato operacional: `AGENTS.md`.
