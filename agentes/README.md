# Execuções de agentes

Esta raiz reserva diretórios locais e ignorados para execuções auditáveis. Cada
tarefa usa um subdiretório próprio para entradas, estado, logs, temporários,
artefatos, target Cargo e worktree. Somente esta política e o `.gitignore` são
versionados; material de execução nunca entra na PR.
