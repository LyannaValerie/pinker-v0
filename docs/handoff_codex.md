# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 90 — indexação mínima de `verso` com fronteira auditável (`indice_verso`)**.
- Rodada funcional pequena, focada em fechar o item 5 do Bloco 8 sem expandir biblioteca textual ampla.

## 2. O que entrou na rodada atual
- Inclusão da intrínseca `indice_verso(verso, bombom) -> verso` no recorte operacional de `--run`.
- Diagnóstico explícito de fronteira para indexação de `verso`: tipo inválido (semântica) e índice fora da faixa (runtime).
- Cobertura com testes de semântica/`--run`/CLI e exemplo versionado `examples/fase90_verso_indexacao_minima_valido.pink`.
- Atualização de `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md` e `docs/phases.md` para continuidade factual da trilha.

## 3. Fora de escopo da rodada atual
- Slicing de `verso`, indexação negativa, replace/split, formatação/interpolação.
- Ampliação de gramática para `v[i]` em `verso` nesta fase.
- Biblioteca textual ampla ou redesign de runtime.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **item 6 do Bloco 8 (`falar`)**, mantendo recorte pequeno e auditável.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **90**.
- Rodada documental mais recente: **Doc-15**.
- Hotfix extraordinário mais recente preservado: **HF-2 (Bloco 6, Fases 64–70)**.
- Hotfix histórico extraordinário preservado: **HF-1 (Fase 48-H1)**.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
