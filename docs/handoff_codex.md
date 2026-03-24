# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 108 — append textual mínimo em `--run`**.
- Rodada funcional pequena, local e auditável no Bloco 8.

## 2. O que entrou na rodada atual
- Intrínsecas novas de arquivo/texto: `abrir_anexo(verso) -> bombom` e `anexar_verso(bombom, verso) -> nulo`.
- Semântica mínima da fase: append textual por handle aberto com `abrir_anexo`, sem newline implícito e sem append por caminho.
- Alinhamento de semântica/IR/CFG IR/selected/Machine/validações/runtime para reconhecer as duas intrínsecas sem declaração explícita.
- Cobertura nova em semântica + runtime + CLI com exemplo versionado da fase 108.
- Atualização documental mínima: `README.md`, `docs/roadmap.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md`, `docs/phases.md` e correção pontual de drift em `docs/ponte_engine_rosa.md`.

## 3. Continuidade preservada
- Fase funcional atual passa a **108**.
- Fase funcional anterior passa a **107**.
- Bloco ativo permanece **Bloco 8**.
- Recorte de arquivo/texto segue mínimo (sem append por caminho, sem modos ricos, sem streaming, sem escrita por linha e sem biblioteca textual ampla).

## 4. Próximo item normal
- Seguir refinamentos mínimos e auditáveis no Bloco 8 sem inflar API textual.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
