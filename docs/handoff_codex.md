# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 177 — linguagem-cola: argv explícito mínimo para executar_com_entrada (camada 4 conservadora)**.
- Leitura operacional canônica: **Fase 177** concluída; **Bloco 17** permanece encerrado por suficiência conservadora; **Bloco 16** segue como frente funcional oficialmente ativa.

## 2. Resultado operacional da rodada
- `executar_com_entrada` passou a aceitar exatamente um `argv1` textual explícito opcional além de `comando` e `entrada`, preservando stdin textual mínimo e retorno por código de saída.
- A ampliação foi aplicada só no ponto assimétrico restante da família de subprocessos, com cobertura em semântica, runtime, CLI e exemplos versionados da fase.
- `pipeline_minimo` permanece fora desta expansão; múltiplos argumentos, shell implícito, quoting rico, stdin adulto e redesign geral de processos continuam fora do recorte.

## 3. Próximo passo correto
- Próximo passo funcional provável: retomar **16.2 — linguagem-cola**.
- Blocos **18** e **19** permanecem apenas como candidatos futuros; não foram abertos nesta rodada.

## 4. Restrições explícitas
- Sem tratar o Bloco 17 encerrado como frente funcional; 18 e 19 seguem apenas como candidatos futuros.
- Sem implementar reformas sintáticas (novas keywords, inferência local, `;` opcional, etc.) sem abertura funcional explícita de fase.
- Sem inflar `future.md` como se fosse roadmap ou `parallel.md` como se fosse backlog técnico executável.
- Sem reabrir subprocessos além do recorte explícito desta fase nem expandir `pipeline_minimo` por inércia.
