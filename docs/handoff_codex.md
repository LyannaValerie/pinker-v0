# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 99 — refinamento mínimo de mutação de filesystem em `--run` (`criar_diretorio` + `remover_arquivo`)**.
- Rodada funcional curta do Bloco 8 com foco em mutação mínima controlada de filesystem sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `criar_diretorio(verso) -> nulo` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`) para criação simples de diretório (não recursiva).
- Intrínseca `remover_arquivo(verso) -> nulo` adicionada no mesmo recorte mínimo para remoção simples de arquivo.
- Exemplo versionado novo: `examples/fase99_refinamento_diretorio_arquivo_minimo_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: criação positiva, remoção positiva, negativo de tipo/caminho inadequado e integração com `argumento_ou`/`juntar_caminho`/`caminho_existe`/`e_arquivo`/`e_diretorio`/`falar`.

## 3. Fora de escopo da rodada atual
- timestamps/permissões/ownership, criação recursiva, remoção de diretório, rename/move/cópia e listagem de diretórios.
- Processos externos ou biblioteca ampla de filesystem/metadados.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **99**.
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
