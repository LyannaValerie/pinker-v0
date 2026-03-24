# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 109 — leitura textual mínima direta por caminho em `--run`**.
- Rodada funcional pequena, local e auditável no Bloco 8.

## 2. O que entrou na rodada atual
- Intrínsecas novas de arquivo/texto: `ler_arquivo_verso(verso) -> verso` e `arquivo_ou(verso, verso) -> verso`.
- Semântica mínima da fase: leitura textual completa direta por caminho; fallback textual mínimo por caminho para ausência/impossibilidade simples de leitura no runtime.
- Alinhamento de semântica/IR/CFG IR/selected/Machine/validações/runtime para reconhecer as duas intrínsecas sem declaração explícita.
- Cobertura nova em semântica + runtime + CLI com exemplo versionado da fase 109.
- Atualização documental mínima: `README.md`, `docs/roadmap.md`, `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/vocabulario.md`, `docs/phases.md` e higiene de referência `MANUAL.md` no `docs/atlas.md`.

## 3. Continuidade preservada
- Fase funcional atual passa a **109**.
- Fase funcional anterior passa a **108**.
- Bloco ativo permanece **Bloco 8**.
- Recorte de arquivo/texto segue mínimo (sem streaming, sem escrita/append por caminho, sem modos ricos, sem seek/cursor público e sem biblioteca textual ampla).

## 4. Próximo item normal
- Seguir refinamentos mínimos e auditáveis no Bloco 8 sem inflar API textual.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
