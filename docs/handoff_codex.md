# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 74 — convenção de chamada concreta mínima**.
- Segunda fase funcional do Bloco 7, com chamada real mínima no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` evoluiu do subset linear sem chamadas (Fase 73) para subset com **call direta real** em Linux x86_64.
- Convenção concreta mínima documentada e implementada no emissor externo: `%rdi` para argumento único `bombom` e `%rax` para retorno `bombom`.
- Backend externo passou a aceitar múltiplas funções `-> bombom` em bloco único linear, com `principal()` mapeada para `main`.
- Teste de integração externa real ampliado para caso com função auxiliar + call + execução binária (exit code validado).
- Exemplos versionados da Fase 74 adicionados para caso positivo (call com 1 argumento) e negativo (2 argumentos fora do subset).

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- Mais de 1 parâmetro, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **Frame/registradores mínimos reais**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **74**.
- Fase funcional anterior: **73**.
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
