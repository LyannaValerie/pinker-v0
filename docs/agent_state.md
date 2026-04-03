# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase mais recente: **181 — core nobre e bibliotecas temáticas: definição das famílias temáticas oficiais**.
- Bloco oficialmente ativo na trilha canônica: **18 — core nobre e bibliotecas temáticas**.
- Bloco documental mais recentemente encerrado: **17 — forma visual e superfície documental (encerrado por suficiência conservadora na Fase 176)**.
- Bloco anteriormente ativo já encerrado por suficiência conservadora: **16 — ferramenta cotidiana madura e linguagem-cola (encerrado na Fase 179)**.
- Camada atual do bloco em curso: **18 — documental/arquitetural, sem operacionalização nova em código**.
- Bloco encerrado: **11 — texto prático, scripts e ergonomia cotidiana (encerrado por suficiência conservadora na Doc-27)**.
- Bloco encerrado: **12 — sistema de módulos tipado (encerrado por suficiência conservadora na Doc-28)**.
- Bloco encerrado: **13 — coleções e estruturas de dados básicas (encerrado por suficiência conservadora na Fase 156)**.
- Bloco encerrado: **14 — formatação e dados estruturados (encerrado por suficiência conservadora na Doc-29, após as Fases 157, 158, 159 e 160)**.
- Bloco funcional imediatamente anterior já consolidado: **15 — processos e integração sistêmica**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Rodada documental mais recente: **Fase 181 — definição das famílias temáticas oficiais do Bloco 18**.
- Ajuste extraordinário corrente: promoção canônica de `tem_chave`, `pedir_argumento` e `buscar_contexto`, com legado temporário para `tem_argumento_nomeado`, `argumento_nomeado_ou` e `argumento_nomeado_ou_ambiente_ou`.
- Leitura canônica do estado: a Fase 176 encerrou o **Bloco 17** por suficiência conservadora; as Fases 168, 169, 170 e 177 cumpriram o recorte mínimo plausível de **16.2**; a Fase 179 encerrou o **Bloco 16** por suficiência conservadora; a Fase 180 abriu o **Bloco 18** com inventário canônico de intrínsecas; e a Fase 181 continuou o **Bloco 18** ao canonizar suas famílias públicas iniciais sem abrir operacionalização em código.
- Síntese consolidada do Bloco 17: norma visual mínima, uniformização inicial de exemplos canônicos, refinamento mínimo de tom documental, convenção mínima para `trazer`/uso qualificado e política mínima para aliases e nomes curtos.
- Limite canônico do fechamento: o Bloco 17 não abriu sintaxe nova, reforma de keywords, inferência local, `;` opcional, unidade implícita, redesign de módulos nem qualquer mudança funcional em parser, semântica, runtime, `src/`, `tests/` ou compatibilidade da linguagem.
- Síntese atual do Bloco 18: famílias públicas iniciais aceitas (`texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`), domínios provisórios explícitos (`colecao`, `formato`) e recomendação de `tempo` como família exemplar do bloco.
- Limite canônico de 18.2: a Pinker ainda não possui `familia.intrinseca`, `trazer familia;`, `trazer familia.intrinseca;` nem qualquer reorganização funcional da engine ligada a famílias.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.
- Escada interna consolidada do Bloco 15: **15.1 concluído no recorte mínimo; 15.2 concluído no recorte mínimo; 15.3 concluído no recorte mínimo; 15.4 concluído no recorte mínimo; 15.5 concluído no recorte mínimo (`pipeline_minimo`)**.
- Escada interna consolidada do Bloco 16: **16.1 concluído no recorte mínimo (`pink repl`); 16.2 concluído por suficiência conservadora com as Fases 168, 169, 170 e 177, consolidado no fechamento do bloco pela Fase 179**.
- Limite canônico de 16.2: `pipeline_minimo` permaneceu fora da expansão de `argv1` explícito; continuam fora múltiplos argv gerais, shell implícito, quoting/escaping rico, stdin adulto, PTY, job control e shell rica.
- Próximo passo documental provável no Bloco 18: detalhar a família exemplar (`tempo`) e preparar a superfície futura por família sem fingir mecanismo já implementado.

## 3. Arquitetura documental ativa
- `roadmap.md` = ordem ativa.
- `history.md` = crônica única.
- `agent_state.md` = estado corrente enxuto.
- `handoff_codex.md` = bilhete operacional curto.
- `atlas.md` = navegação mestre.
- `ponte_engine_rosa.md` = mediação estável Engine ↔ Rosa.
- `inventario_intrinsecas.md` = inventário canônico de intrínsecas (Bloco 18).
- `phases.md` = compatibilidade legada.

## 4. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.
- Não operacionalizar famílias públicas antes da decisão lexical de 18.2.

## 5. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP histórico (`pinker_mcp`) foi removido por segurança e não faz parte do estado operacional atual.
- Padrão recomendado: `cargo run --bin pink -- ...`.
