# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 143 — ergonomia prática de script: prioridade mínima entre argumento nomeado e ambiente (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- Novo intrínseco de runtime `argumento_nomeado_ou_ambiente_ou(chave_arg, chave_env, padrao) -> verso` implementado no `--run`.
- Prioridade mínima fechada e auditável para script real: argumento nomeado, depois ambiente, depois fallback textual explícito.
- Rejeições explícitas preservadas: chave de argumento vazia, chave de ambiente vazia e `--chave` sem valor continuam falhando com erro claro.
- Cobertura adicionada com testes semânticos, de runtime e de CLI, além de exemplo versionado da fase.

## 3. Próximo passo correto
- Seguir a trilha funcional já aberta no Bloco 11, mantendo fases pequenas e auditáveis após a Fase 143.

## 4. Restrições explícitas
- Sem abrir fase funcional nova nesta rodada documental.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
