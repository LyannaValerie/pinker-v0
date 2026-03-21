# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 70 — indexação operacional em arrays**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 69.

## 2. O que entrou na rodada atual
- Leitura operacional mínima por índice em array no `--run`, via caminho `(*ptr)[i]`.
- Lowering de `Index` passou a usar ponteiro base + offset do índice + `deref_load` no modelo de memória atual.
- Subset explícito desta fase: `ptr: seta<[bombom; N]>` e índice `i: bombom`.
- Novos testes positivos/negativos e exemplos versionados da Fase 70.

## 3. Fora de escopo da rodada atual
- Escrita operacional por índice (`arr[i] = v`).
- Base por valor em indexação (`arr[i]` sem ponteiro).
- Indexação operacional para elementos não `bombom`.
- Efeito operacional robusto de `fragil` (MMIO/barreiras).
- Backend nativo real de memória/ponteiros.
- Operações de ponteiro além do subset mínimo já existente (`n + ptr`, `ptr - ptr`, comparações ricas de ponteiros).

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **cast operacional útil ligado à memória (item B.8 do Bloco 6)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **70**.
- Fase funcional anterior: **69**.
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
