# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 106 — normalização mínima de caixa em `verso` no `--run`**.
- Rodada funcional pequena, local e auditável no Bloco 8.

## 2. O que entrou na rodada atual
- Intrínsecas novas de `verso`: `minusculo_verso(verso) -> verso` e `maiusculo_verso(verso) -> verso`.
- Alinhamento de semântica/IR/CFG IR/selected/Machine/validações/runtime para reconhecer as duas intrínsecas sem declaração explícita.
- Cobertura nova em semântica + runtime + CLI com exemplo versionado da fase 106.
- Atualização documental mínima: `README.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md` e `docs/phases.md`.

## 3. Continuidade preservada
- Fase funcional atual passa a **106**.
- Fase funcional anterior passa a **105**.
- Bloco ativo permanece **Bloco 8**.
- Recorte textual segue mínimo (sem casefolding, sem locale-aware e sem biblioteca textual ampla).

## 4. Próximo item normal
- Seguir refinamentos mínimos e auditáveis no Bloco 8 sem inflar API textual.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
