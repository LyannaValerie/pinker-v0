# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 98 — refinamento mínimo de arquivo em `--run` (`tamanho_arquivo` + `e_vazio`)**.
- Rodada funcional curta do Bloco 8 com foco em metadados mínimos de arquivo sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `tamanho_arquivo(verso) -> bombom` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`) para tamanho mínimo de arquivo regular.
- Intrínseca `e_vazio(verso) -> logica` adicionada no mesmo recorte mínimo para teste de vazio em arquivo regular.
- Exemplo versionado novo: `examples/fase98_refinamento_arquivo_minimo_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: tamanho positivo, vazio positivo, negativo com caminho ausente e integração com `argumento_ou`/`juntar_caminho`/`caminho_existe`/`e_arquivo`/`falar`.

## 3. Fora de escopo da rodada atual
- timestamps/permissões/ownership, criação/remoção de arquivo, listagem de diretórios e leitura incremental.
- Processos externos ou biblioteca ampla de filesystem/metadados.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **98**.
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
