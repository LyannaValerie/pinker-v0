# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 101 — escrita textual mínima de arquivo em `--run` (`escrever_verso` + `criar_arquivo`)**.
- Rodada funcional curta do Bloco 8 com foco em escrita textual mínima + complemento operacional de criação sem abrir API ampla.

## 2. O que entrou na rodada atual
- Intrínseca `escrever_verso(handle, verso) -> nulo` adicionada ao pipeline completo (semântica, IR, validações, runtime `--run`) para sobrescrita textual integral em handle aberto.
- Intrínseca `criar_arquivo(verso) -> bombom` adicionada no mesmo recorte mínimo para criação de arquivo vazio com retorno imediato de handle.
- Exemplo versionado novo: `examples/fase101_escrita_textual_minima_arquivo_valido.pink`.
- Cobertura de testes ampliada em semântica e `--run`/CLI para: escrita textual positiva, releitura positiva, negativo de handle inválido e integração com `argumento_ou`/`juntar_caminho`/`caminho_existe`/`e_arquivo`.

## 3. Fora de escopo da rodada atual
- timestamps/permissões/ownership, criação recursiva, remoção recursiva, rename/move/cópia e listagem de diretórios.
- Processos externos ou biblioteca ampla de filesystem/metadados.
- streaming, append, escrita por linha e API textual rica de arquivo.
- Redesign de runtime ou expansão de gramática.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **manter refinamentos mínimos de tooling/I/O em `--run` preservando escopo pequeno e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **101**.
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
