# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase funcional mais recente: **206 — expansão de coleções e ergonomia: tipo `mapa<bombom,verso>`**.
- Rodada documental mais recente: **Doc-38 — restauração do mapa macro do Bloco 18** (anterior à Fase 186).
- Bloco oficialmente ativo na trilha canônica: **18 — core nobre e bibliotecas temáticas**.
- Bloco documental mais recentemente encerrado: **17 — forma visual e superfície documental (encerrado por suficiência conservadora na Fase 176)**.
- Bloco anteriormente ativo já encerrado por suficiência conservadora: **16 — ferramenta cotidiana madura e linguagem-cola (encerrado na Fase 179)**.
- Camada atual do bloco em curso: **18 — recorte funcional mínimo de importação por família ampliado até a Fase 189 (18.6); Fases 190–206 expandiram ergonomia e coleções fora do escopo formal do bloco**.
- Bloco encerrado: **11 — texto prático, scripts e ergonomia cotidiana (encerrado por suficiência conservadora na Doc-27)**.
- Bloco encerrado: **12 — sistema de módulos tipado (encerrado por suficiência conservadora na Doc-28)**.
- Bloco encerrado: **13 — coleções e estruturas de dados básicas (encerrado por suficiência conservadora na Fase 156)**.
- Bloco encerrado: **14 — formatação e dados estruturados (encerrado por suficiência conservadora na Doc-29, após as Fases 157, 158, 159 e 160)**.
- Bloco funcional imediatamente anterior já consolidado: **15 — processos e integração sistêmica**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Ajuste extraordinário corrente: **arquitetura do roadmap shardada e AX-friendly em `docs/roadmap/`**, sem mudança funcional de linguagem.
- Leitura canônica do estado: a Fase 176 encerrou o **Bloco 17** por suficiência conservadora; as Fases 168, 169, 170 e 177 cumpriram o recorte mínimo plausível de **16.2**; a Fase 179 encerrou o **Bloco 16** por suficiência conservadora; a Fase 180 abriu o **Bloco 18** com inventário canônico de intrínsecas; a Fase 181 continuou o **Bloco 18** ao canonizar suas famílias públicas iniciais; a Fase 182 formalizou `tempo` como família exemplar do bloco sobre a superfície mínima já existente; a Fase 183 formalizou a ponte canônica entre a superfície global legada e a futura superfície por família a partir desse caso exemplar; a Fase 184 formalizou o domínio interno por intrínseca como leitura documental estável do inventário; a Fase 185 formalizou a leitura preparatória da futura resolução qualificada por família sem abrir mecanismo operacional; a Fase 186 abriu o primeiro recorte funcional real de 18.6 com `trazer tempo;` validado pelo checker e pelo runtime, preservando a compatibilidade global legada integralmente; a Fase 187 ampliou esse mesmo mecanismo no menor recorte auditável para `trazer ambiente;`, também sem tornar o import obrigatório nem quebrar a superfície global legada; a Fase 188 ampliou esse mesmo mecanismo, ainda no menor recorte funcional sustentável, para `trazer acaso;`, preservando integralmente a superfície global legada; a Fase 189 estende esse mesmo mecanismo para `trazer texto;`, com os nomes textuais canônicos globais permanecendo válidos sem import; a Doc-35 reduziu o custo de contexto do histórico sem alterar código; a Doc-36 reduziu o gigantismo do roadmap ao separá-lo em topo executivo, índice e shards por bloco; as Fases 190–202 expandiram a ergonomia e expressividade da linguagem com comentários de bloco, sequências de escape, operadores compostos, novas intrínsecas utilitárias, literais negativos, strings multiline, `repetir...até`, `para...de...até`, `eterno` para verso, operador ternário, `escolha/caso/padrao`, retorno implícito e interpolação de verso; as Fases 203–206 expandiram o domínio provisório `colecao` com `lista<verso>`, `mapa<verso,verso>`, `mapa<bombom,bombom>` e `mapa<bombom,verso>`, incluindo intrínsecas e iteração via `para cada` para cada tipo.
- Síntese consolidada do Bloco 17: norma visual mínima, uniformização inicial de exemplos canônicos, refinamento mínimo de tom documental, convenção mínima para `trazer`/uso qualificado e política mínima para aliases e nomes curtos.
- Limite canônico do fechamento: o Bloco 17 não abriu sintaxe nova, reforma de keywords, inferência local, `;` opcional, unidade implícita, redesign de módulos nem qualquer mudança funcional em parser, semântica, runtime, `src/`, `tests/` ou compatibilidade da linguagem.
- Síntese atual do Bloco 18: famílias públicas iniciais aceitas (`texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`), domínios provisórios explícitos (`colecao`, `formato`), formalização de `tempo` como família exemplar do bloco sobre `tempo_unix` e `formatar_tempo_unix`, política documental explícita para a futura superfície por família, classificação canônica de domínio interno por intrínseca, preparação canônica da futura resolução qualificada por família e abertura de um recorte mínimo funcional de importação por família agora cobrindo `trazer tempo;`, `trazer ambiente;`, `trazer acaso;` e `trazer texto;` (Fases 186–189). As Fases 190–206 expandiram funcionalidade fora do escopo formal do bloco: ergonomia de linguagem e ampliação do domínio provisório `colecao` com 4 novos tipos de coleção e 34 novas intrínsecas públicas.
- Limite canônico de 18.6 (Fases 186–189): `trazer tempo;`, `trazer ambiente;`, `trazer acaso;` e `trazer texto;` funcionam; `trazer familia.simbolo;` não suportado; outras famílias não importáveis; compatibilidade global legada preservada; sem modo estrito; sem reorganização de engine.
- Limite canônico das Fases 190–206: sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação de tipo é monomorphizada; sem coleções heterogêneas; todos os novos tipos seguem o padrão monomorphizado existente de `lista<bombom>` e `mapa<verso,bombom>`.
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) com trilha MCP mínima posteriormente removida por segurança**.
- Último hotfix aplicado: **HF-5 — ajuste de conformidade Clippy pós-Fase 136**.
- Escada interna consolidada do Bloco 15: **15.1 concluído no recorte mínimo; 15.2 concluído no recorte mínimo; 15.3 concluído no recorte mínimo; 15.4 concluído no recorte mínimo; 15.5 concluído no recorte mínimo (`pipeline_minimo`)**.
- Escada interna consolidada do Bloco 16: **16.1 concluído no recorte mínimo (`pink repl`); 16.2 concluído por suficiência conservadora com as Fases 168, 169, 170 e 177, consolidado no fechamento do bloco pela Fase 179**.
- Limite canônico de 16.2: `pipeline_minimo` permaneceu fora da expansão de `argv1` explícito; continuam fora múltiplos argv gerais, shell implícito, quoting/escaping rico, stdin adulto, PTY, job control e shell rica.
- Próximo passo provável: continuar avançando funcionalidades de alta dificuldade restantes ou retomar o eixo 18.6/18.7 do Bloco 18.

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
- Binário MCP local (`pinker_mcp`) existe novamente como servidor stdio zero-dependency e restrito ao projeto. A superfície atual é apenas de leitura/análise (`pinker_checar`, `pinker_tokens`, `pinker_render`); não expõe execução `--run`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
