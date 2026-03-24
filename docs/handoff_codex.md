# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 100 — remoção mínima complementar de diretório + leitura textual mínima de arquivo em `--run` (`remover_diretorio` + `ler_verso_arquivo`)**.
- Rodada funcional curta do Bloco 8 com foco em complemento mínimo de mutação de diretório e leitura textual simples sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `remover_diretorio(verso) -> nulo` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`) para remoção simples de diretório vazio (não recursiva).
- Intrínseca `ler_verso_arquivo(handle) -> verso` adicionada no mesmo recorte mínimo para leitura textual integral de arquivo já aberto com `abrir(...)`.
- Exemplo versionado novo: `examples/fase100_refinamento_diretorio_texto_minimo_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: remoção positiva de diretório vazio, negativo de diretório não-vazio, leitura textual positiva e integração com `argumento_ou`/`juntar_caminho`/`caminho_existe`/`e_diretorio`/`falar`.

## 3. Fora de escopo da rodada atual
- timestamps/permissões/ownership, criação recursiva, remoção recursiva, rename/move/cópia e listagem de diretórios.
- Processos externos ou biblioteca ampla de filesystem/metadados.
- streaming, append e API textual rica de arquivo.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **100**.
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
