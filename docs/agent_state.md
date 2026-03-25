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
- Fase funcional atual: **114 — globais mínimas e base inicial de `.rodata` no backend nativo externo**.
- Fase funcional anterior: **113 — loops reais mínimos no backend nativo externo**.
- Bloco ativo: **Bloco 9 — ampliação do backend nativo real** (Bloco 8 encerrado formalmente por suficiência de trilha).
- Rodada documental mais recente: **Doc-19 — encerramento formal do Bloco 8 e abertura canônica do Bloco 9**.
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
4. Próxima rodada normal esperada: evolução do Bloco 9 para o item 9.5 (ABI mínima mais larga, ainda conservadora), mantendo escopo mínimo e sem reabrir refinamentos do Bloco 8 sem necessidade extraordinária.

## 9. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP: `pinker_mcp`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
- `default-run = "pink"` preserva ergonomia.
