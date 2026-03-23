# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 93 — ergonomia mínima de argv em `--run` (contagem + presença por índice)**.
- Rodada funcional pequena, focada em script/tool simples sem abrir parser amplo de CLI.

## 2. O que entrou na rodada atual
- Nova intrínseca `quantos_argumentos() -> bombom` para contagem mínima de argv em `--run`.
- Nova intrínseca `tem_argumento(bombom) -> logica` para verificação mínima de presença por índice.
- Integração explícita com `argumento(i)` já existente para fluxo de guarda mínima antes da leitura posicional.
- Cobertura com testes de semântica/`--run`/CLI e exemplo versionado `examples/fase93_argv_ergonomia_minima_valido.pink`.
- Atualização de `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md`, `docs/roadmap.md` e `docs/phases.md` para continuidade factual da trilha.

## 3. Fora de escopo da rodada atual
- Parser de flags/subcomandos/env vars para tooling.
- Biblioteca utilitária ampla de CLI/argv.
- Diretórios, processos externos, pipes e backend externo para tooling.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **seguir refinamentos pequenos de tooling útil sem inflar escopo de CLI**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **93**.
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
