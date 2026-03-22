# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 77 — memória real mínima no backend**.
- Quinta fase funcional do Bloco 7, consolidando o primeiro recorte explícito de load/store real em memória de frame no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` preserva o recorte de chamadas da Fase 76 e passa a declarar explicitamente memória mínima real em frame: load/store de slots via endereçamento `%rbp`.
- Header textual do subset externo e mensagens de erro foram alinhados para Fase 77, mantendo o contrato conservador do backend externo montável.
- Testes de integração externa real ganharam caso versionado dedicado da fase cobrindo fluxo com escrita/leitura de slots de frame e validação de resultado executável.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- Mais de 2 parâmetros, parâmetros não `bombom`, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **Artefato executável mais amplo**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **77**.
- Fase funcional anterior: **76**.
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
