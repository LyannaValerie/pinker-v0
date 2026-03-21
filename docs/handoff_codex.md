# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 71 — cast operacional útil ligado à memória**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 70.

## 2. O que entrou na rodada atual
- Lowering operacional de `virar` passou a existir em CFG/selected/Machine/runtime para subset mínimo útil de memória.
- Subset explícito desta fase: inteiro->inteiro e `bombom <-> seta<bombom>`.
- Runtime agora executa cast explícito com reinterpretação de endereço lógico (`cast`) para o subset suportado.
- Novos testes positivos/negativos e exemplos versionados da Fase 71.

## 3. Fora de escopo da rodada atual
- `bombom -> seta<T>` genérico e `seta<T> -> bombom` para `T != bombom`.
- Cast geral entre tipos compostos/ponteiros.
- Efeito operacional robusto de `fragil` (MMIO/barreiras).
- Backend nativo real de memória/ponteiros.
- Operações de ponteiro além do subset mínimo já existente (`n + ptr`, `ptr - ptr`, comparações ricas de ponteiros).

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **primeiro efeito operacional real de `fragil` (item B.9 do Bloco 6)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **71**.
- Fase funcional anterior: **70**.
- Hotfix extraordinário mais recente: **HF-2 (Bloco 6, Fases 64–70)** — varredura de corretude pós-Bloco-6.
  - Bug corrigido: `normalize_numeric_pair` invertia ordem de operandos (signed/unsigned misto).
  - Bug corrigido: `Eq/Neq` no IR e CFG IR validator rejeitava `signed_var == literal`.
  - Diagnóstico melhorado: classificador de erros de runtime cobre erros de ponteiro.
  - Código morto removido: verificação redundante em `semantic.rs` `ExprKind::Index`.
  - Regressão adicionada: `run_signed_literal_lhs_operacoes_nao_comutativas`.
- Hotfix anterior: **HF-1 (Fase 48-H1)**.
- Rodadas documentais seguem sem numeração de fase funcional.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
