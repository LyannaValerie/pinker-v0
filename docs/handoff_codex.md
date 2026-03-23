# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 97 — refinamento mínimo de caminho em `--run` (`e_diretorio` + `juntar_caminho`)**.
- Rodada funcional curta do Bloco 8 com foco em classificação complementar + composição mínima de caminho sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `e_diretorio(verso) -> logica` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`) para classificação complementar de diretório.
- Intrínseca `juntar_caminho(verso, verso) -> verso` adicionada no mesmo recorte mínimo, usando composição por infraestrutura de path da stdlib.
- Exemplo versionado novo: `examples/fase97_refinamento_caminho_minimo_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: diretório existente, negativos em arquivo/ausente, composição mínima e integração com `diretorio_atual`/`argumento_ou`/`caminho_existe`/`falar`.

## 3. Fora de escopo da rodada atual
- canonicalização/normalização rica, listagem de diretórios, `chdir`, globbing e mutação ampla de paths.
- Processos externos ou biblioteca ampla de filesystem.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **97**.
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
