# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 96 — introspecção mínima de caminho em `--run` (`caminho_existe` + `e_arquivo`)**.
- Rodada funcional curta do Bloco 8 com foco em existência/classificação mínima de caminho sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `caminho_existe(verso) -> logica` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`).
- Intrínseca `e_arquivo(verso) -> logica` adicionada ao mesmo recorte mínimo para classificação de arquivo.
- Exemplo versionado novo: `examples/fase96_introspeccao_caminho_minima_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: caminho existente, caminho ausente e integração com `diretorio_atual`/`falar`.

## 3. Fora de escopo da rodada atual
- `e_diretorio`, listagem de diretórios, `chdir`, globbing e mutação de paths.
- Processos externos ou biblioteca ampla de filesystem.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **96**.
- Rodada documental mais recente preservada: **Doc-17**.
- Rodada paralela concluída preservada: **Paralela-1** — negação bitwise dual (`~` + `nope`) + MCP mínimo (`src/bin/pinker_mcp.rs`).
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
