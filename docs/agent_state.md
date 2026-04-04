# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase funcional mais recente: **182 — core nobre e bibliotecas temáticas: família exemplar `tempo`**.
- Rodada documental mais recente: **Doc-36 — refatoração estrutural AX-friendly do roadmap**.
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
- Ajuste extraordinário corrente: **arquitetura do roadmap shardada e AX-friendly em `docs/roadmap/`**, sem mudança funcional de linguagem.
- Leitura canônica do estado: a Fase 176 encerrou o **Bloco 17** por suficiência conservadora; as Fases 168, 169, 170 e 177 cumpriram o recorte mínimo plausível de **16.2**; a Fase 179 encerrou o **Bloco 16** por suficiência conservadora; a Fase 180 abriu o **Bloco 18** com inventário canônico de intrínsecas; a Fase 181 continuou o **Bloco 18** ao canonizar suas famílias públicas iniciais; a Fase 182 formalizou `tempo` como família exemplar do bloco sobre a superfície mínima já existente; a Doc-35 reduziu o custo de contexto do histórico sem alterar código; e a Doc-36 reduziu o gigantismo do roadmap ao separá-lo em topo executivo, índice e shards por bloco.
- Síntese consolidada do Bloco 17: norma visual mínima, uniformização inicial de exemplos canônicos, refinamento mínimo de tom documental, convenção mínima para `trazer`/uso qualificado e política mínima para aliases e nomes curtos.
- Limite canônico do fechamento: o Bloco 17 não abriu sintaxe nova, reforma de keywords, inferência local, `;` opcional, unidade implícita, redesign de módulos nem qualquer mudança funcional em parser, semântica, runtime, `src/`, `tests/` ou compatibilidade da linguagem.
- Síntese atual do Bloco 18: famílias públicas iniciais aceitas (`texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`), domínios provisórios explícitos (`colecao`, `formato`) e formalização de `tempo` como família exemplar do bloco sobre `tempo_unix` e `formatar_tempo_unix`.
- Limite canônico de 18.2: a Pinker ainda não possui `familia.intrinseca`, `trazer familia;`, `trazer familia.intrinseca;` nem qualquer reorganização funcional da engine ligada a famílias.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.
- Escada interna consolidada do Bloco 15: **15.1 concluído no recorte mínimo; 15.2 concluído no recorte mínimo; 15.3 concluído no recorte mínimo; 15.4 concluído no recorte mínimo; 15.5 concluído no recorte mínimo (`pipeline_minimo`)**.
- Escada interna consolidada do Bloco 16: **16.1 concluído no recorte mínimo (`pink repl`); 16.2 concluído por suficiência conservadora com as Fases 168, 169, 170 e 177, consolidado no fechamento do bloco pela Fase 179**.
- Limite canônico de 16.2: `pipeline_minimo` permaneceu fora da expansão de `argv1` explícito; continuam fora múltiplos argv gerais, shell implícito, quoting/escaping rico, stdin adulto, PTY, job control e shell rica.
- Próximo passo documental provável no Bloco 18: continuar a preparação da superfície futura por família a partir do caso exemplar `tempo`, sem reabsorver o papel estrutural recém-isolado em `docs/roadmap/` nem fingir resolução qualificada já pronta.

## 3. Arquitetura documental ativa
- `roadmap.md` = ordem ativa.
- `roadmap/indice.md` = hub curto de navegação por blocos do roadmap.
- `roadmap/blocos/*.md` = detalhe estrutural por bloco.
- `history.md` = ponteiro canônico curto do histórico.
- `history/indice.md` = hub histórico principal.
- `history/*/indice.md` = roteadores locais por categoria.
- `history/*/*.md` = shards factuais do histórico.
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
