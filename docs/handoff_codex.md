# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 88 — `verso` operacional útil mínimo em `--run`**.
- Rodada funcional mínima do Bloco 8, focada em oficializar `verso` como valor operacional de runtime sem abrir operações de texto.

## 2. O que entrou na rodada atual
- `verso` passou a ser operacional no recorte mínimo de `--run`: variável local, passagem por chamada, retorno e `falar(verso)` por valor.
- CFG IR passou a lowerar valor string em expressões para `OperandIR::Str` (sem liberar `eterno` global de `verso` nesta fase).
- Machine/runtime ganharam caminho de impressão de `verso` por valor de pilha (`print_str_value`) além do caminho prévio para literal imediato.
- Testes e exemplo versionado adicionados: `examples/fase88_verso_operacional_minimo_valido.pink` + casos positivos em `--run` e `--cfg-ir`.

## 3. Fora de escopo da rodada atual
- Modos de abertura, append, truncate selecionável e streaming.
- Diretórios e API rica de filesystem.
- concatenação/comprimento/indexação de `verso`.
- `eterno` global de `verso` em CFG IR/runtime.
- formatação/interpolação e API textual ampla.

## 4. Próximo item normal
- Trilha ativa: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **operações mínimas de texto** (`verso` concatenação/comprimento/indexação) com recorte mínimo e auditável.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **88**.
- Fase funcional anterior: **87**.
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
