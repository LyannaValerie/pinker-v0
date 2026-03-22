# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 75 — frame/registradores mínimos reais**.
- Terceira fase funcional do Bloco 7, consolidando disciplina mínima de registradores/frame no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` manteve o subset funcional da Fase 74 e adotou disciplina explícita mínima de registradores/frame no recorte Linux x86_64.
- Papéis fixos nesta fase: `%rax` para acumulador/retorno, `%rdi` para argumento único e `%r10` como temporário volátil para binárias lineares (`+`, `-`, `*`), removendo uso ad hoc de registrador callee-saved.
- Frame mínimo por função preservado com `%rbp` e slots lineares para parâmetro/local/temporários, com emissão mais consistente entre atribuição, aritmética e call.
- Testes de integração externa real cobrem chamada/retorno e locals/aritmética sob a nova disciplina, incluindo novo exemplo versionado da Fase 75.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- Mais de 1 parâmetro, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **Chamadas reais no subset nativo**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **75**.
- Fase funcional anterior: **74**.
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
