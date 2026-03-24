# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 110 — entrada textual mínima em `--run`**.
- Rodada funcional pequena, local e auditável no Bloco 8.

## 2. O que entrou na rodada atual
- Intrínsecas novas de entrada textual: `ouvir_verso() -> verso` e `ouvir_verso_ou(verso) -> verso`.
- Semântica mínima da fase: leitura textual única da stdin com remoção mínima de newline final; fallback textual mínimo para EOF/impossibilidade operacional simples em `ouvir_verso_ou`.
- Alinhamento de semântica/IR/CFG IR/selected/Machine/validações/runtime para reconhecer as duas intrínsecas sem declaração explícita.
- Cobertura nova em semântica + runtime + CLI com exemplo versionado da fase 110.
- Atualização documental mínima: `README.md`, `docs/roadmap.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md`, `docs/phases.md` e `docs/ponte_engine_rosa.md`.

## 3. Continuidade preservada
- Fase funcional atual passa a **110**.
- Fase funcional anterior passa a **109**.
- Bloco ativo permanece **Bloco 8**.
- Recorte de entrada textual segue mínimo (sem streaming, sem API rica de terminal, sem timeout/leitura não bloqueante, sem encoding sofisticado e sem biblioteca textual ampla).

## 4. Próximo item normal
- Seguir refinamentos mínimos e auditáveis no Bloco 8 sem inflar API textual.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
