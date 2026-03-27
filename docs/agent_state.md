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
- Fase funcional atual: **136 — abertura funcional do editor/TUI oficial da Pinker (camada 1 conservadora)**.
- Fase funcional anterior: **135 — `verso` mínima (camada 1 conservadora e condicional) no backend nativo externo**.
- Frente ativa: **Bloco 11 — texto prático, scripts e ergonomia cotidiana**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Bloco ativo de compilador/backend/ecossistema: **Bloco 11** (aberto canonicamente na Doc-25).
- Rodada documental mais recente: **Doc-25 — abertura canônica do Bloco 11**.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) + MCP mínimo**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.

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
4. Próxima rodada normal esperada: abrir a primeira fase funcional do Bloco 11 em recorte pequeno e auditável, começando por texto (11.1), mantendo o editor/TUI pausado nesta transição e sem reabrir o Bloco 10 por impulso.

## 9. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP: `pinker_mcp`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
- `default-run = "pink"` preserva ergonomia.


## 10. Direção estratégica atualizada (Doc-25)
- O editor/TUI oficial da Pinker continua parte oficial do ecossistema e permanece aberto desde a Fase 136, porém pausado por decisão estratégica.
- O Bloco 10 do compilador/backend permanece encerrado por suficiência conservadora (Doc-24), sem reabertura.
- O Bloco 11 foi aberto canonicamente como foco ativo imediato para texto prático, scripts e ergonomia cotidiana.
- Próxima rodada funcional esperada: primeira fase do Bloco 11 começando por manipulação textual útil (11.1), sem continuidade do editor/TUI nesta rodada documental.
