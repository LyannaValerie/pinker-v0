# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 87 — escrita mínima de arquivo com `escrever` em `--run`**.
- Rodada funcional mínima do Bloco 8, focada em escrita simples de arquivo sem inflar API de I/O.

## 2. O que entrou na rodada atual
- Intrínseca `escrever(handle, bombom)` adicionada ao recorte funcional de `--run`, sobrescrevendo conteúdo do arquivo já aberto por `abrir("caminho")`.
- Runtime de arquivo em `--run` passou a manter caminho + conteúdo por handle, permitindo `escrever` seguido de `ler_arquivo` no mesmo handle.
- Pipeline de semântica/IR/CFG/selected/Machine/validações passou a reconhecer `escrever` como intrínseca do subset da fase.
- Testes e exemplo versionado adicionados: `examples/fase87_arquivo_escrita_minima_valido.pink` + casos de sucesso/falha no `--run`.

## 3. Fora de escopo da rodada atual
- Modos de abertura, append, truncate selecionável e streaming.
- Diretórios e API rica de filesystem.
- Escrita textual ampla com `verso`.
- `verso` operacional amplo em runtime (passagem geral por chamada/retorno/variável).

## 4. Próximo item normal
- Trilha ativa: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **verso operacional útil** com recorte mínimo e auditável.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **87**.
- Fase funcional anterior: **86**.
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
