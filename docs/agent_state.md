# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase mais recente: **172 — forma visual e superfície documental: uniformização mínima de exemplos canônicos**.
- Bloco documental mais recentemente aberto: **17 — forma visual e superfície documental**.
- Frente funcional oficialmente ativa: **16 — ferramenta cotidiana madura e linguagem-cola**.
- Bloco encerrado: **11 — texto prático, scripts e ergonomia cotidiana (encerrado por suficiência conservadora na Doc-27)**.
- Bloco encerrado: **12 — sistema de módulos tipado (encerrado por suficiência conservadora na Doc-28)**.
- Bloco encerrado: **13 — coleções e estruturas de dados básicas (encerrado por suficiência conservadora na Fase 156)**.
- Bloco encerrado: **14 — formatação e dados estruturados (encerrado por suficiência conservadora na Doc-29, após as Fases 157, 158, 159 e 160)**.
- Bloco funcional imediatamente anterior já consolidado: **15 — processos e integração sistêmica**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Rodada documental mais recente: **Doc-32 — abertura documental da trilha de superfície Pinker: direções candidatas (Blocos 17, 18 e 19)**.
- Ajuste extraordinário corrente: promoção canônica de `tem_chave`, `pedir_argumento` e `buscar_contexto`, com legado temporário para `tem_argumento_nomeado`, `argumento_nomeado_ou` e `argumento_nomeado_ou_ambiente_ou`.
- Leitura canônica do estado: as Fases 171 e 172 mantêm o **Bloco 17** em camada documental, sem deslocar a frente funcional oficial do **Bloco 16**.
- Entrega documental mais recente do Bloco 17: um lote pequeno e visível de exemplos canônicos foi uniformizado segundo `docs/style.md`, começando por `examples/run_soma.pink`, `examples/run_chamada.pink`, `examples/run_global.pink` e `examples/fase89_verso_operacoes_minimas_valido.pink`.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.
- Escada interna consolidada do Bloco 15: **15.1 concluído no recorte mínimo; 15.2 concluído no recorte mínimo; 15.3 concluído no recorte mínimo; 15.4 concluído no recorte mínimo; 15.5 concluído no recorte mínimo (`pipeline_minimo`)**.
- Próximo passo funcional provável: **16.2 segue como continuação da frente funcional ativa**, em camadas conservadoras pequenas com argv explícito camada 1 em `executar_processo`, camada 2 em `capturar_stdout` e camada 3 em `capturar_stderr`.

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
