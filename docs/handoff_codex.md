# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 155 — correção conservadora da iteração mínima sobre `mapa<verso,bombom>` sem expor chave por índice como intrínseca geral**.
- **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase funcional ativa passa a ser 155 no Bloco 13.
- O construto `para cada chave em mapa { ... }` está operacional para `mapa<verso,bombom>` com variável de chave `verso` no corpo e valor via `mapa_verso_bombom_obter`.
- `mapa_verso_bombom_tamanho` permanece público, mas a dependência de `mapa_verso_bombom_chave_indice` foi removida da superfície semântica pública.
- O lowering de mapa passa a usar cursor interno com snapshot de chaves no runtime, preservando a superfície `para cada chave em mapa { ... }` com custo conceitual menor.
- Rastreamento mínimo de tipo de coleção no parser continua permitindo dispatch correto entre lista e mapa sem redesign amplo de AST.
- `lista<bombom>` continua funcional; regressão zero confirmada.

## 3. Próximo passo correto
- Evoluir o Bloco 13 para o próximo degrau mínimo auditável (13.9 — aleatoriedade básica), sem inflar coleções, iteração genérica ou API rica.

## 4. Restrições explícitas
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
