# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 69 — acesso a campo operacional em `ninho`**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 68.

## 2. O que entrou na rodada atual
- Leitura operacional mínima de campo em `ninho` no `--run`, via caminho `(*ptr).campo` com `ptr: seta<ninho>`.
- Lowering de `FieldAccess` passou a usar offset do layout estático (`layout`) + aritmética de ponteiro + `deref_load`.
- Subset explícito de campo nesta fase: apenas campos escalares (`bombom`, `u8..u64`, `i8..i64`, `logica`).
- Novos testes positivos/negativos e exemplos versionados da Fase 69.

## 3. Fora de escopo da rodada atual
- Escrita operacional de campo em `ninho`.
- Acesso por valor (`p.campo`) e formas fora do padrão `(*ptr).campo`.
- Indexação operacional plena.
- Efeito operacional robusto de `fragil` (MMIO/barreiras).
- Backend nativo real de memória/ponteiros.
- Operações de ponteiro além do subset mínimo já existente (`n + ptr`, `ptr - ptr`, comparações ricas de ponteiros).

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **indexação operacional em arrays (item B.7 do Bloco 6)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **69**.
- Fase funcional anterior: **68**.
- Hotfix extraordinário preservado: **HF-1 (Fase 48-H1)**.
- Rodadas documentais seguem sem numeração de fase funcional.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `phases.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
