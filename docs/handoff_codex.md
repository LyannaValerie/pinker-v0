# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 84 — recusa explícita complementar de `sempre que` no subset externo**.
- Rodada funcional pequena e auditável no Bloco 7, sem abertura de fundamento novo e sem abrir Bloco 8.

## 2. O que entrou na rodada atual
- Backend externo `--asm-s` (`emit_external_toolchain_subset`) passou a recusar explicitamente `sempre que` com diagnóstico dedicado.
- Contrato textual do subset externo foi atualizado para Fase 84 e agora declara também a recusa explícita de loops.
- Testes do subset externo foram atualizados para Fase 84 e ganharam caso negativo dedicado para `sempre que`.
- Exemplo versionado novo: `examples/fase84_backend_externo_recusa_explicita_sempre_que_invalido.pink`.
- Documentação mínima sincronizada: `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/phases.md`.

## 3. Fora de escopo da rodada atual
- Abertura de suporte parcial a loops no backend externo.
- Lowering novo de labels/jumps/branches.
- ABI ampla, register allocation amplo, memória indireta/ponteiros, 3+ parâmetros e novos tipos no subset externo.
- Abertura do Bloco 8.

## 4. Próximo item normal
- Trilha ativa: **Bloco 7 — Backend nativo real**.
- Próximo item funcional sugerido: **continuidade conservadora do artefato executável com reforços explícitos pequenos de fronteira (sem abrir fundamentos novos)**.

## 5. Observações operacionais curtas
- Fase funcional atual: **84**.
- Fase funcional anterior: **83**.
- Hotfix extraordinário mais recente preservado: **HF-2 (Bloco 6, Fases 64–70)**.
- Hotfix histórico extraordinário preservado: **HF-1 (Fase 48-H1)**.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
