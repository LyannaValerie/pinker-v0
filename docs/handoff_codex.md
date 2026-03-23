# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 91 — melhorias mínimas em `falar` (múltiplos argumentos + mistura heterogênea mínima)**.
- Rodada funcional pequena, focada em abrir ergonomia de saída sem abrir subsistema de formatação.

## 2. O que entrou na rodada atual
- `falar(...)` passou a aceitar múltiplos argumentos no `--run`.
- Separação previsível por espaço simples entre argumentos e quebra de linha única no fim da chamada.
- Mistura mínima heterogênea coberta no recorte da fase (incluindo `verso` + `bombom` na mesma chamada).
- Cobertura com testes de `--run`/CLI e exemplo versionado `examples/fase91_falar_multiplos_argumentos_valido.pink`.
- Atualização de `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md` e `docs/phases.md` para continuidade factual da trilha.

## 3. Fora de escopo da rodada atual
- Interpolação/formatação ampla, placeholders nomeados/posicionais, largura/alinhamento/precisão.
- Biblioteca textual ampla ou redesign de runtime.
- Backend externo para saída formatada.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **item 7 do Bloco 8 (base para tooling em Pinker)**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **91**.
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
