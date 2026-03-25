# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Diretrizes consolidadas de execução
- Manter fases pequenas, auditáveis e coerentes com `docs/roadmap.md`.
- Evitar refactor amplo fora do escopo da rodada.
- Preservar continuidade histórica e não reabrir fase concluída.
- Em conflito, código mergeado prevalece sobre documentação.

## 3. Convenção de fases/rodadas
- **Fase N**: entrega funcional real.
- **HF-N**: hotfix extraordinário sem abrir nova fase funcional.
- **Doc-N**: rodada exclusivamente documental.
- **Paralela-N**: rodada paralela de implementação.
- Histórico detalhado: `docs/history.md`.

## 4. Pipeline congelada
- Fluxo base: semântica -> IR -> validação IR -> CFG IR -> validação CFG -> selected -> validação selected -> Machine -> validação Machine.
- Saídas: `--pseudo-asm`, `--asm-s`, `--run`.

## 5. Estado corrente
- Fase funcional atual: **119 — compostos mínimos (camada 4 conservadora, fechamento do recorte homogêneo) no backend nativo externo**.
- Fase funcional anterior: **118 — compostos mínimos (camada 3 conservadora) no backend nativo externo**.
- Bloco ativo: **nenhum bloco funcional em continuidade automática** (Bloco 9 encerrado conservadoramente como trilha ativa; Bloco 8 já encerrado).
- Rodada documental mais recente: **Doc-20 — encerramento conservador do Bloco 9**.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) + MCP mínimo**.
- Último hotfix aplicado: **HF-3 — estabilização do Bloco 8 (Fases 85–101)**.

## 6. Arquitetura documental dual ativa
- Navegação mestre: `docs/atlas.md`.
- Hemisfério Engine: `roadmap`, `history`, `agent_state`, `handoff`, `doc_rules`, `future`.
- Hemisfério Rosa: `rosa`, `vocabulario`, `parallel`.
- Documento-ponte: `docs/ponte_engine_rosa.md`.

## 7. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.

## 8. Instrução para novo agente
1. Ler: `README.md`, `docs/atlas.md`, `docs/roadmap.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/history.md`, `docs/doc_rules.md`.
2. Executar validações exigidas da rodada antes de encerrar.
3. Atualizar ao final: `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` e `docs/phases.md` quando houver mudança documental/operacional.
4. Próxima rodada normal esperada: definir conscientemente a próxima frente funcional, sem continuação automática do item 9.6 e sem reabrir o Bloco 9 por inércia; qualquer reabertura deve ser extraordinária, pequena e bem justificada.

## 9. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP: `pinker_mcp`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
- `default-run = "pink"` preserva ergonomia.
