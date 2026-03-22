# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Doc-12 — sincronização ampla de `docs/future.md` com o estado real do projeto**.
- Rodada exclusivamente documental; sem mudança funcional, sem abertura de fase, sem alteração do roadmap ativo.

## 2. O que entrou na rodada atual
- `docs/future.md` revisado de ponta a ponta: itens com drift documental marcados corretamente como ✅ implementado ou 🔶 parcial, com referência às fases correspondentes.
- Principais correções: `fragil` (🔶, Fases 52+72), dereferência/aritmética de ponteiros (🔶, Fases 66–68), acesso operacional a campo/índice (🔶, Fases 69–70), `virar` lowering (🔶, Fase 71), `trazer` (🔶, Fase 60), geração x86_64/ABI (🔶, Bloco 7 Fases 73–83), `sussurro`/`livre;`/linker script (🔶, Fases 56–58), `falar`/`verso`/`pink build` (🔶, Fases 61–63), inteiros signed runtime (✅, Fase 64).
- Intro corrigido: "próxima trilha ativa" → "trilha ativa corrente" para Bloco 7.
- Frentes prioritárias: texto atualizado para refletir status 🔶 atual de cada item.
- `docs/vocabulario.md`: `seta` movida para keywords implementadas (Fase 48).
- `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` atualizados para a rodada Doc-12.

## 3. Fora de escopo da rodada atual
- Implementação de funcionalidade nova.
- Abertura de fase funcional ou bloco novo.
- Alteração do roadmap ativo.
- Mudança na filosofia ou ordem de execução do projeto.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **continuidade conservadora do artefato executável com reforços explícitos pequenos de fronteira (sem abrir fundamentos novos)**.
- Bloco 8 aguarda consolidação suficiente do Bloco 7 antes de ser aberto como trilha ativa.

## 5. Observações operacionais curtas
- Fase funcional atual: **83**.
- Fase funcional anterior: **82**.
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
