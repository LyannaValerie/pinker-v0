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
- Frente ativa: **editor/TUI oficial da Pinker**.
- Bloco ativo de compilador/backend: **nenhum** (Bloco 10 segue encerrado desde a Doc-24).
- Rodada documental mais recente: **Doc-24 — encerramento conservador do Bloco 10 e liberação estratégica da trilha do editor/TUI**.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) + MCP mínimo**.
- Último hotfix aplicado: **HF-4 — varredura completa de repositório e higiene estrutural pós-B9**.

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
4. Próxima rodada normal esperada: evoluir o editor/TUI em recortes pequenos e auditáveis, sem transformar em IDE ampla e sem reabrir o Bloco 10 por impulso.

## 9. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP: `pinker_mcp`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
- `default-run = "pink"` preserva ergonomia.


## 10. Direção estratégica atualizada (Fase 136)
- O editor/TUI oficial da Pinker deixou de ser apenas direção liberada e passou a existir funcionalmente em camada 1 conservadora.
- O Bloco 10 do compilador/backend permanece encerrado por suficiência conservadora (Doc-24), sem reabertura nesta fase.
- Ordem estratégica explícita: evoluir editor/TUI por camadas pequenas, com integração real ao fluxo Pinker e sem inflar escopo para IDE ampla.
