# Boundary de Tasks

Esta raiz contém somente o boundary versionado das Tasks. A implementação e o
lifecycle da Forja vivem no host.

```text
ONE_TASK -> ONE_OBSERVED_TASK_ROOT
ALL_DISPOSABLE_TASK_RESOURCES ⊂ TASK_ROOT
```

Somente esta política e o `.gitignore` são versionados; dados de execução não
entram na PR. O slot físico não é o `TASK_ID`: observe a identidade corrente com
`forja-agentes observe --json`.

Autoridade host-side: `/pinker/playground/ferramentas-agente/`.
Provisionamento, verificação, selo e retirada usam exclusivamente
`forja-agentes` e o lifecycle suportado pelo host.
