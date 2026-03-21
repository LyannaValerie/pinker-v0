# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 72 — efeito operacional mínimo de `fragil`**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 71.

## 2. O que entrou na rodada atual
- `fragil` passou a ter efeito operacional mínimo real em acesso indireto (`*p` e `*p = valor`) com caminhos distintos no pipeline/runtime.
- Metadata de volatilidade (`is_volatile`) agora é propagada e validada de IR até Machine para `deref_load`/`deref_store`.
- Subset explícito desta fase: `fragil seta<bombom>` no `--run` (leitura e escrita indiretas).
- Novos testes positivos/negativos e exemplos versionados da Fase 72.

## 3. Fora de escopo da rodada atual
- MMIO real, hardware real, fences/barreiras e semântica sofisticada de ordenação de memória para `fragil`.
- Ampliação agressiva de subset de `fragil` para bases além de `bombom`.
- Backend nativo real de memória/ponteiros.

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **expandir o subset operacional de `fragil` de forma incremental, sem abrir MMIO/fences**.

## 5. Observações operacionais curtas
- Fase funcional atual: **72**.
- Fase funcional anterior: **71**.
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
