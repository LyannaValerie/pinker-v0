# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 66 — dereferência de leitura**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 65.

## 2. O que entrou na rodada atual
- Sintaxe de dereferência de leitura `*expr` no frontend.
- Validação semântica para dereferência no subset da fase: apenas `seta<bombom>`.
- Lowering operacional em IR/CFG/selected/Machine/runtime com instrução dedicada (`deref_load`).
- Runtime com memória abstrata mínima de leitura indireta baseada em endereços de globals escalares (`eterno`) para permitir execução em `--run`.
- Aceite de literal inteiro como endereço de bootstrap para inicializar `seta<T>` nesta fase.
- Novos testes positivos/negativos e exemplos versionados da Fase 66.

## 3. Fora de escopo da rodada atual
- Escrita indireta (`*p = v`).
- Aritmética de ponteiros.
- Acesso operacional de campo/index por ponteiro.
- Efeito operacional robusto de `fragil` (MMIO/barreiras).
- Backend nativo real de memória/ponteiros.

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **escrita indireta via ponteiro (item B.4 do Bloco 6)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **66**.
- Fase funcional anterior: **65**.
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
