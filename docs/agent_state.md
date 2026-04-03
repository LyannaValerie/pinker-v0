# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase mais recente: **179 — ferramenta cotidiana madura e linguagem-cola: fechamento do Bloco 16 por suficiência conservadora**.
- Bloco documental mais recentemente encerrado: **17 — forma visual e superfície documental (encerrado por suficiência conservadora na Fase 176)**.
- Frente funcional oficialmente ativa: **nenhuma nova frente aberta nesta rodada; Bloco 16 encerrado por suficiência conservadora na Fase 179**.
- Bloco encerrado: **11 — texto prático, scripts e ergonomia cotidiana (encerrado por suficiência conservadora na Doc-27)**.
- Bloco encerrado: **12 — sistema de módulos tipado (encerrado por suficiência conservadora na Doc-28)**.
- Bloco encerrado: **13 — coleções e estruturas de dados básicas (encerrado por suficiência conservadora na Fase 156)**.
- Bloco encerrado: **14 — formatação e dados estruturados (encerrado por suficiência conservadora na Doc-29, após as Fases 157, 158, 159 e 160)**.
- Bloco funcional imediatamente anterior já consolidado: **15 — processos e integração sistêmica**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Rodada documental mais recente: **Doc-32 — abertura documental da trilha de superfície Pinker: direções candidatas (Blocos 17, 18 e 19)**.
- Ajuste extraordinário corrente: promoção canônica de `tem_chave`, `pedir_argumento` e `buscar_contexto`, com legado temporário para `tem_argumento_nomeado`, `argumento_nomeado_ou` e `argumento_nomeado_ou_ambiente_ou`.
- Leitura canônica do estado: a Fase 176 encerrou o **Bloco 17** por suficiência conservadora; as Fases 168, 169, 170 e 177 cumpriram o recorte mínimo plausível de **16.2**; a Fase 178 encerrou essa subtrilha por suficiência conservadora; e a Fase 179 consolidou o fechamento do **Bloco 16** por suficiência conservadora.
- Síntese consolidada do Bloco 17: norma visual mínima, uniformização inicial de exemplos canônicos, refinamento mínimo de tom documental, convenção mínima para `trazer`/uso qualificado e política mínima para aliases e nomes curtos.
- Limite canônico do fechamento: o Bloco 17 não abriu sintaxe nova, reforma de keywords, inferência local, `;` opcional, unidade implícita, redesign de módulos nem qualquer mudança funcional em parser, semântica, runtime, `src/`, `tests/` ou compatibilidade da linguagem.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.
- Escada interna consolidada do Bloco 15: **15.1 concluído no recorte mínimo; 15.2 concluído no recorte mínimo; 15.3 concluído no recorte mínimo; 15.4 concluído no recorte mínimo; 15.5 concluído no recorte mínimo (`pipeline_minimo`)**.
- Escada interna consolidada do Bloco 16: **16.1 concluído no recorte mínimo (`pink repl`); 16.2 concluído por suficiência conservadora com as Fases 168, 169, 170 e 177, consolidado documentalmente na Fase 178**.
- Limite canônico de 16.2: `pipeline_minimo` permaneceu fora da expansão de `argv1` explícito; continuam fora múltiplos argv gerais, shell implícito, quoting/escaping rico, stdin adulto, PTY, job control e shell rica.
- Encerramento canônico do Bloco 16 na Fase 179: REPL mínimo auditável + linguagem-cola mínima coerente foram consolidados como arco suficiente no recorte v0, sem abertura de shell adulta, REPL adulto ou integração sistêmica ampla.
- Próximo passo funcional provável: **qualquer expansão pós-Bloco 16 exige abertura explícita de nova trilha**; Blocos 18 e 19 permanecem apenas candidatos futuros, sem promoção automática.

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
