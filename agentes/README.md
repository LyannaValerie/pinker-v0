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

O conjunto de recursos, as classes `EPHEMERAL`/`DURABLE` e a semântica de selo e
retirada pertencem à Forja, não a este repositório. Observe-os na fonte viva, que
é sempre a autoridade corrente:

```bash
forja-agentes observe        # layout em JSON, com classe e presença por recurso
forja-agentes contract       # provisioner_set e finalizer_set
```

Autoridade da implementação: a Forja, no host — fonte e suítes em
`/pinker/playground/ferramentas-agente/` (Issue #544). Contrato de invocação
que a Pinker mantém: `AGENTS.md`, seção *Onde vive a Forja*.
