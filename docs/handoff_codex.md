# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 82 — recusa explícita complementar de controle de fluxo no subset externo**.
- Décima fase funcional do Bloco 7, reforçando de forma conservadora a fronteira do mesmo subset linear no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` foi atualizado para rotular o contrato vigente como Fase 82, preservando o mesmo subset conservador (chamadas diretas lineares, até 2 parâmetros `bombom`, frame `%rbp` e load/store em slots).
- O backend externo passou a recusar explicitamente `talvez/senão` no fluxo `--asm-s`, com diagnóstico dedicado para separar o subset linear garantido de controle de fluxo geral.
- Testes do backend externo ganharam caso negativo versionado dedicado da Fase 82 cobrindo a recusa explícita de `talvez/senão`.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- 3+ parâmetros, parâmetros não `bombom`, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **continuidade conservadora do artefato executável com novos reforços explícitos de fronteira (sem abrir fundamentos novos)**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **82**.
- Fase funcional anterior: **81**.
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
