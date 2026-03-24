# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 107 — observação textual posicional mínima em `verso` no `--run`**.
- Rodada funcional pequena, local e auditável no Bloco 8.

## 2. O que entrou na rodada atual
- Intrínsecas novas de `verso`: `indice_verso_em(verso, verso) -> bombom` e `nao_vazio_verso(verso) -> logica`.
- Contrato mínimo de ausência para busca posicional: `indice_verso_em` retorna sentinela `18446744073709551615` (`u64::MAX`) quando o trecho não é encontrado.
- Alinhamento de semântica/IR/CFG IR/selected/Machine/validações/runtime para reconhecer as duas intrínsecas sem declaração explícita.
- Cobertura nova em semântica + runtime + CLI com exemplo versionado da fase 107.
- Atualização documental mínima: `README.md`, `docs/roadmap.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md` e `docs/phases.md`.

## 3. Continuidade preservada
- Fase funcional atual passa a **107**.
- Fase funcional anterior passa a **106**.
- Bloco ativo permanece **Bloco 8**.
- Recorte textual segue mínimo (sem regex, sem split/replace/slicing geral e sem biblioteca textual ampla).

## 4. Próximo item normal
- Seguir refinamentos mínimos e auditáveis no Bloco 8 sem inflar API textual.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
