# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 67 — escrita indireta**.
- Rodada funcional do Bloco 6, mantendo a trilha principal após a Fase 66.

## 2. O que entrou na rodada atual
- Sintaxe de escrita indireta `*expr = valor;` no frontend, preservando `*expr` de leitura da Fase 66.
- Validação semântica para escrita indireta no subset da fase: apenas `seta<bombom>`.
- Lowering operacional em IR/CFG/selected/Machine/runtime com instrução dedicada (`deref_store`).
- Runtime com atualização da memória abstrata mínima (endereços de globals escalares já mapeadas) para suportar escrita indireta em `--run`.
- Erro de runtime explícito para escrita em endereço inválido ou não inicializado.
- Novos testes positivos/negativos e exemplos versionados da Fase 67.

## 3. Fora de escopo da rodada atual
- Aritmética de ponteiros.
- Acesso operacional de campo/index por ponteiro.
- Efeito operacional robusto de `fragil` (MMIO/barreiras).
- Backend nativo real de memória/ponteiros.

## 4. Próximo item normal
- Trilha ativa: **Bloco 6 — Memória operacional**.
- Próximo item funcional normal sugerido: **aritmética de ponteiros (item B.5 do Bloco 6)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **67**.
- Fase funcional anterior: **66**.
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
