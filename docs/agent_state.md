# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase funcional atual: **148 — coleções e estruturas de dados básicas: escrita mínima por índice em array fixo `[bombom; N]` (camada 1 conservadora)**.
- Bloco encerrado: **11 — texto prático, scripts e ergonomia cotidiana (encerrado por suficiência conservadora na Doc-27)**.
- Bloco encerrado: **12 — sistema de módulos tipado (encerrado por suficiência conservadora na Doc-28)**.
- Bloco formal da trilha ativa: **13 — coleções e estruturas de dados básicas**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Rodada documental mais recente: **Doc-28 — fechamento canônico do Bloco 12 e preparação formal da transição para o Bloco 13** (última rodada documental, sem nova Doc nesta fase funcional).
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.

## 3. Arquitetura documental ativa
- `roadmap.md` = ordem ativa.
- `history.md` = crônica única.
- `agent_state.md` = estado corrente enxuto.
- `handoff_codex.md` = bilhete operacional curto.
- `atlas.md` = navegação mestre.
- `ponte_engine_rosa.md` = mediação estável Engine ↔ Rosa.
- `phases.md` = compatibilidade legada.

## 4. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.

## 5. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP histórico (`pinker_mcp`) foi removido por segurança e não faz parte do estado operacional atual.
- Padrão recomendado: `cargo run --bin pink -- ...`.
