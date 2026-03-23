# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 86 — leitura mínima de arquivo com `abrir`/`fechar` em `--run`**.
- Rodada funcional mínima do Bloco 8, focada em leitura simples de arquivo sem inflar API de I/O.

## 2. O que entrou na rodada atual
- Intrínseca `abrir("caminho") -> bombom` adicionada ao recorte funcional de `--run`, abrindo arquivo por caminho (`verso`) e retornando handle mínimo.
- Intrínseca `ler_arquivo(handle) -> bombom` adicionada para leitura simples de conteúdo inteiro (`u64`) do arquivo aberto.
- Intrínseca `fechar(handle)` adicionada para encerramento explícito do handle no runtime interpretado.
- Pipeline de semântica/IR/CFG/selected/Machine/validações passou a reconhecer `abrir`, `ler_arquivo` e `fechar` como intrínsecas do subset da fase.
- Testes e exemplo versionado adicionados: `examples/fase86_arquivo_leitura_minima_valido.pink` + casos de sucesso/falha no `--run`.

## 3. Fora de escopo da rodada atual
- Escrita de arquivo (`escrever`).
- Múltiplos modos de abertura, streaming incremental, diretórios e API rica de FS.
- Leitura de arquivo para tipos além de inteiro `bombom`.
- `verso` operacional amplo em runtime (passagem geral por chamada/retorno/variável).

## 4. Próximo item normal
- Trilha ativa: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **arquivo — escrita mínima (`escrever`)** no mesmo padrão de diff pequeno e auditável.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **86**.
- Fase funcional anterior: **85**.
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
