# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 80 — cobertura linear auditável mais ampla**.
- Oitava fase funcional do Bloco 7, ampliando de forma conservadora a cobertura executável do mesmo subset linear no backend externo montável.

## 2. O que entrou na rodada atual
- `emit_external_toolchain_subset` foi atualizado para rotular o contrato vigente como Fase 80, preservando o mesmo subset conservador (chamadas diretas lineares, até 2 parâmetros `bombom`, frame `%rbp` e load/store em slots).
- Testes de integração externa real ganharam caso versionado dedicado da Fase 80 cobrindo combinação linear mais rica de densidade local + composição interprocedural no mesmo executável (compilar/montar/linkar/executar + validação de resultado).
- Exemplo versionado da fase foi adicionado para auditoria e reprodução do novo recorte garantido.

## 3. Fora de escopo da rodada atual
- ABI final completa de plataforma.
- 3+ parâmetros, parâmetros não `bombom`, chamadas complexas e recursão externa.
- Lowering de memória indireta/ponteiros no backend externo real.
- Fluxo de controle (`talvez/senão`, loops) no subset externo.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **continuidade conservadora do artefato executável (mais combinações lineares auditáveis sem abrir fundamentos novos)**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **80**.
- Fase funcional anterior: **79**.
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
