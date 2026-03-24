# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 102 — truncamento mínimo de arquivo em `--run`**.
- Rodada funcional pequena e auditável no Bloco 8, com dois subpassos acoplados: truncar por handle e validar explicitamente o pós-estado.

## 2. O que entrou na rodada atual
- Nova intrínseca: `truncar_arquivo(handle) -> nulo` em `--run`, exigindo handle `bombom` aberto.
- Semântica de runtime: truncamento zera o conteúdo persistido em disco e também o buffer associado ao handle aberto.
- Integração explícita do pós-estado com superfícies já existentes:
  - `tamanho_arquivo(verso)` reflete tamanho `0` após truncamento;
  - `e_vazio(verso)` reflete `verdade` após truncamento;
  - `ler_verso_arquivo(handle)` retorna `verso` vazio no mesmo fluxo.
- Cobertura adicionada:
  - testes semânticos para assinatura/tipagem da nova intrínseca;
  - testes de runtime para caso positivo e negativos (handle inválido e handle já fechado);
  - teste CLI com exemplo versionado da fase.

## 3. Fora de escopo da rodada atual
- Sem truncamento por caminho.
- Sem append, sem streaming e sem escrita por linha.
- Sem novos modos de arquivo, sem redesign de runtime e sem biblioteca ampla de filesystem/texto.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item sugerido: manter refinamentos mínimos e locais em `--run`, priorizando observabilidade e previsibilidade de I/O sem ampliar API.

## 5. Observações operacionais curtas
- Fase funcional atual: **102**.
- Fase funcional anterior: **101**.
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
