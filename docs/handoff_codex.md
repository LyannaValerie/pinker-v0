# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 85 — entrada básica com `ouvir` em `--run`**.
- Rodada funcional mínima do Bloco 8, com foco exclusivo em entrada padrão para um tipo básico.

## 2. O que entrou na rodada atual
- Intrínseca `ouvir()` adicionada ao recorte funcional de `--run`, com leitura de `stdin` para `bombom` e erro claro em entrada inválida.
- Semântica/IR/validações passaram a reconhecer `ouvir` como intrínseca de aridade zero sem exigir declaração de função.
- Testes adicionados para caso positivo e erro de parse em runtime via CLI com `stdin` controlado.
- Exemplo versionado da fase incluído: `examples/fase85_ouvir_bombom_valido.pink`.
- Documentação atualizada para registrar a Fase 85 e manter o Bloco 8 como trilha ativa.

## 3. Fora de escopo da rodada atual
- I/O de arquivo (`abrir`, `fechar`, `escrever`).
- `verso` operacional amplo (passagem por chamada/retorno/variável em runtime).
- Suporte de `ouvir` para múltiplos tipos além de `bombom`.
- Backend externo para I/O.

## 4. Próximo item normal
- Trilha ativa: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **arquivo mínimo (`abrir`/`fechar` + leitura simples) ou ampliação controlada de `ouvir` para um segundo tipo, mantendo diff pequeno**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **85**.
- Fase funcional anterior: **84**.
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
