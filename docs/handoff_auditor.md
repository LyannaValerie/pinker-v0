# Handoff Auditor

## Rodada atual
- **Fase 38**: comentários de `--machine` sensíveis ao papel do fluxo.

## Escopo auditado
- Apenas renderização textual de `--machine`.
- Sem mudanças na semântica, parser, lowering, interpretador, Machine/opcodes ou flags.
- `--selected` permaneceu inalterado.

## Continuidade histórica
- Sequência validada e preservada: 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 → 31 → 32 → 33 → 34 → 35 → 36 → 37 → 38.

## Evidências técnicas
- `br_true` com contexto de `if`: comentário focado em ramo `talvez`/`senão`.
- `br_true` com contexto de loop: comentário focado em continuar iteração vs sair do loop.
- `br_true` de curto-circuito: comentário focado em atalho lógico vs avaliar RHS.
- `jmp` com alvo `join_*`/`logic_join_*`: comentário focado em convergência/continuação.
- anotações de bloco `join_*`/`logic_join_*` ajustadas para papel de retomada.

## Arquivos auditados
- `src/abstract_machine.rs`
- `tests/abstract_machine_tests.rs`
- `docs/phases.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`
- `docs/handoff_auditor.md`
