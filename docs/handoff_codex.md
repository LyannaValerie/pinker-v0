# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 105 — saneamento textual mínimo em `--run`**.
- Rodada funcional pequena e auditável no Bloco 8, com dois subpassos acoplados: detecção de vazio textual e aparo mínimo de bordas.

## 2. O que entrou na rodada atual
- Novas intrínsecas: `vazio_verso(verso) -> logica` e `aparar_verso(verso) -> verso` em `--run`.
- Semântica de runtime:
  - `vazio_verso` verifica vazio textual exato (sem aparo implícito);
  - `aparar_verso` remove whitespace nas bordas e retorna `verso`.
- Integração explícita com superfícies já existentes:
  - fluxo com `ler_verso_arquivo(handle)` para saneamento mínimo de conteúdo lido de arquivo;
  - fluxo com `escrever_verso(handle, verso)` + `falar(...)` para decisão/saída em script mínimo.
- Cobertura adicionada:
  - testes semânticos para assinatura/tipagem das duas intrínsecas;
  - testes de runtime para casos positivos/negativos de vazio e aparo;
  - teste de integração com leitura textual de arquivo (incluindo arquivo com whitespace);
  - teste CLI com exemplo versionado da fase.

## 3. Fora de escopo da rodada atual
- Sem split/replace/regex/trim.
- Sem biblioteca textual ampla.
- Sem redesign de runtime.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item sugerido: manter refinamentos mínimos e locais em `--run`, priorizando observabilidade e previsibilidade sem ampliar API textual.

## 5. Observações operacionais curtas
- Fase funcional atual: **105**.
- Fase funcional anterior: **104**.
- Rodada documental mais recente preservada: **Doc-17**.
- Rodada paralela concluída preservada: **Paralela-1** — negação bitwise dual (`~` + `nope`) + MCP mínimo (`src/bin/pinker_mcp.rs`).
- Hotfix extraordinário mais recente preservado: **HF-3 (Bloco 8, Fases 85–101)**.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
