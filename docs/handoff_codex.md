# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 76 — múltiplos parâmetros mínimos reais**.
- Quarta fase funcional do Bloco 7, ampliando a convenção concreta mínima de chamadas no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` ampliou o subset funcional para suportar chamadas diretas com até 2 parâmetros `bombom` no recorte Linux x86_64 hospedado.
- Convenção concreta mínima desta fase: `%rdi` (arg0), `%rsi` (arg1), `%rax` (retorno/acumulador) e `%r10` (temporário volátil de binárias lineares `+`, `-`, `*`).
- Frame mínimo por função foi preservado com `%rbp` e slots lineares para parâmetro/local/temporários.
- Testes de integração externa real agora cobrem fluxo compilável/executável com função de 2 parâmetros `bombom` e retorno calculado.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- Mais de 2 parâmetros, parâmetros não `bombom`, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **Memória real mínima no backend**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **76**.
- Fase funcional anterior: **75**.
- Hotfix extraordinário mais recente: **HF-2 (Bloco 6, Fases 64–70)** — varredura de corretude pós-Bloco-6.
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
