# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Doc-17 — alinhamento documental/operacional pós-Paralela-1 (binários + MCP)**.
- Rodada curta sem nova fase funcional, focada em coerência documental e saneamento de uso de binários.

## 2. O que entrou na rodada atual
- `docs/doc_rules.md` passou a formalizar rodadas paralelas de implementação como categoria própria na crônica (`RODADAS PARALELAS`), distinta de Fase/HF/Doc.
- `docs/future.md` foi corrigido para refletir o estado real: Bloco 8 ativo; Bloco 7 consolidado para transição, não ativo.
- Ambiguidade de `cargo run` (dois binários: `pink` e `pinker_mcp`) foi saneada com duas camadas: `default-run = "pink"` em `Cargo.toml` e atualização dos comandos do README para `cargo run --bin pink -- ...`.
- README recebeu observação operacional curta sobre o binário `pinker_mcp` e seu uso mínimo via JSON-RPC 2.0 em stdio.
- Verificação prática do `pinker_mcp` confirmou `initialize`, `tools/list` e `tools/call` (`pinker_rodar`) funcionando com mensagens JSON por linha.

## 3. Fora de escopo da rodada atual
- Nova feature de linguagem, runtime ou backend nativo.
- Reimplementação/expansão do MCP além da inspeção prática do estado já existente.
- Redesign de CLI e reescrita ampla de documentação.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **seguir trilha do Bloco 8 com refinamentos mínimos de I/O/tooling em `--run`, mantendo escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **95**.
- Rodada documental mais recente: **Doc-17**.
- Rodada paralela concluída: **Paralela-1** — negação bitwise dual (`~` + `nope`) + MCP mínimo (`src/bin/pinker_mcp.rs`).
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
