# Handoff Codex (operacional curto)

## 1. Rodada atual
- **HF-3 — estabilização do Bloco 8 (Fases 85–101): handles, I/O, caminho, texto**.
- Rodada de estabilização/hotfix sem nova feature funcional, com foco em reproduzir bugs, corrigir diagnósticos de handle e ampliar cobertura de testes de borda.

## 2. O que entrou na rodada atual
- Correção de diagnóstico: uso de handle após `fechar` agora produz mensagem específica "handle já fechado" em vez de "handle inválido" genérico (5 intrínsecas afetadas: `ler_arquivo`, `ler_verso_arquivo`, `escrever`, `escrever_verso`, `fechar`).
- Rastreio de handles fechados (`closed_handles: HashSet<u64>`) em `RuntimeIoState`.
- Classificador de erros atualizado com categoria `handle_ja_fechado` e dica diagnóstica.
- 11 testes novos de borda/estabilização no `interpreter_tests.rs`.

## 3. Fora de escopo da rodada atual
- Nenhuma nova intrínseca, feature funcional ou modo de arquivo.
- Redesign de runtime, pipeline ou gramática.
- Expansão de superfície de I/O além do recorte Fase 101.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **101**.
- Rodada documental mais recente preservada: **Doc-17**.
- Rodada paralela concluída preservada: **Paralela-1** — negação bitwise dual (`~` + `nope`) + MCP mínimo (`src/bin/pinker_mcp.rs`).
- Hotfix extraordinário mais recente: **HF-3 (Bloco 8, Fases 85–101)**.
- Hotfixes históricos preservados: **HF-2 (Bloco 6, Fases 64–70)**, **HF-1 (Fase 48-H1)**.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
