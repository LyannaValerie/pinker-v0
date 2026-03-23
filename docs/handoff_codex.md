# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 92 — base mínima para tooling em `--run` (argv posicional + status explícito)**.
- Rodada funcional pequena, focada em script/tool simples sem abrir parser amplo de CLI.

## 2. O que entrou na rodada atual
- `--run` passou a aceitar `-- <args...>` para repasse de argumentos posicionais do host para o script.
- Nova intrínseca `argumento(bombom) -> verso` no runtime para leitura posicional mínima de argv.
- Nova intrínseca `sair(bombom)` para status/código explícito de saída em scripts/ferramentas simples.
- Cobertura com testes de semântica/`--run`/CLI e exemplo versionado `examples/fase92_tooling_base_argumento_status_valido.pink`.
- Atualização de `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md` e `docs/phases.md` para continuidade factual da trilha.

## 3. Fora de escopo da rodada atual
- Parser de flags/subcomandos/env vars para tooling.
- Biblioteca utilitária ampla de CLI/argv.
- Diretórios, processos externos, pipes e backend externo para tooling.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **consolidar ergonomia mínima pós-Fase 92 sem inflar escopo de tooling**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **92**.
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
