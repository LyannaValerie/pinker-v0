# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 73 — subset real montável ampliado**.
- Primeira fase funcional do Bloco 7, com mudança real no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` evoluiu de retorno constante para subset linear maior em Linux x86_64: locais `bombom`, atribuição local e aritmética `+`, `-`, `*` com retorno calculado.
- Emissão externa passou a gerar frame mínimo real (prólogo/epílogo + slots de stack para locais/temporários).
- Teste de integração externa real ampliado para caso com múltiplas instruções escalares e retorno calculado.
- Exemplos versionados adicionados para caso positivo/negativo do novo subset externo.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- Parâmetros, chamadas e fluxo de controle no subset externo montável.
- Lowering de memória indireta/ponteiros no backend externo real.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **Convenção de chamada concreta mínima**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **73**.
- Fase funcional anterior: **72**.
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
