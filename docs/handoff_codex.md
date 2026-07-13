# Estado operacional da Pinker v0

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente

| Campo | Valor |
|---|---|
| Fase funcional mais recente | **224** — Eixo A: propagação explícita de error handling com `propagar` |
| Rodada documental mais recente | **Doc-42** — referência `expandir.md`, novo padrão pós-Eixo B e remoção de referências ativas a `docs/phases.md` |
| Bloco ativo | **20** — expansão funcional rumo a SO e self-hosting (trilha por faixas) |
| Último bloco encerrado | **18** — core nobre e bibliotecas temáticas (Fase 207) |
| Frente pausada | editor/TUI oficial da Pinker (Fase 136) |
| Última rodada paralela | **Paralela-1** — negação bitwise dual |
| Último hotfix | **HF-5** — conformidade Clippy pós-Fase 136 |

### Blocos encerrados

| Bloco | Nome | Encerramento |
|---|---|---|
| 11 | texto prático, scripts e ergonomia cotidiana | Doc-27 |
| 12 | sistema de módulos tipado | Doc-28 |
| 13 | coleções e estruturas de dados básicas | Fase 156 |
| 14 | formatação e dados estruturados | Doc-29 (Fases 157–160) |
| 15 | processos e integração sistêmica | consolidado |
| 16 | ferramenta cotidiana madura e linguagem-cola | Fase 179 |
| 17 | forma visual e superfície documental | Fase 176 |
| 18 | core nobre e bibliotecas temáticas | Fase 207 |

### Fases recentes

| Fases | Contribuição |
|---|---|
| 180–185 | Bloco 18, abertura documental: inventário de intrínsecas, famílias públicas, `tempo` exemplar, domínios internos |
| 186–189 | 18.6: `trazer tempo;`, `trazer ambiente;`, `trazer acaso;`, `trazer texto;` |
| 190–202 | Ergonomia: comentários de bloco, escape sequences, operadores compostos, intrínsecas utilitárias, literais negativos, multiline strings, `repetir...até`, `para...de...até`, `eterno` verso, ternário, `escolha/caso`, retorno implícito, interpolação |
| 203–206 | Coleções: `lista<verso>`, `mapa<verso,verso>`, `mapa<bombom,bombom>`, `mapa<bombom,verso>` |
| 207 | 18.6 concluído: `trazer arquivo;`, `trazer caminho;`, `trazer processo;`; fechamento do Bloco 18; abertura do Bloco 20 |
| 208 | Bloco 20, Faixa 1, item 1 (recorte mínimo): `leque` — enum nominal estilo C |
| 209 | Bloco 20, Faixa 1: carga por variante (`bombom`/`verso`) + `encaixe` com exaustividade; **primeiro degrau do Marco self-hosting 1 verificado** (lexer de brinquedo em Pinker) |
| 210 | Bloco 20, Faixa 1: múltiplas cargas + carga de tipo leque (recursão e recursão mútua); **fundação do Marco self-hosting 2 verificada** (avaliador recursivo de AST em Pinker) |
| 211 | Bloco 20, Faixa 1: `lista<T>` genérica sobre leques + 7 intrínsecas genéricas; **Marco self-hosting 2 verificado em miniatura** (compilador de brinquedo lexer→parser→avaliador em Pinker) |
| 212 | Bloco 20, Eixo B (B1): workspace com runtime nativo `pinker_rt` (staticlib ABI C, alocador testado) + `pink build --nativo` produzindo ELF real linkado ao runtime |
| 213 | Bloco 20, Eixo B (B2): ABI SysV completa — 6 registradores + args de pilha com padding, N parâmetros, recursão e chamadas aninhadas nativas |
| 214 | Bloco 20, Eixo B (B3): controle de fluxo geral nativo — todos os construtos de fluxo executam nativos; ternário abaixa para `cmov` |
| 215 | Bloco 20, Eixo B (B4): `verso` dinâmico nativo — layout length-prefixed único, `juntar`/`tamanho`/`igual` + `falar` completo via runtime, **paridade de stdout verificada** |
| 216 | Bloco 20, Eixo B (B5): listas nativas completas — runtime unificado (elementos = palavras de 8 bytes) servindo `lista<bombom>`/`lista<verso>`/`lista<Leque>`, `para cada` nativo, paridade de stdout |
| 217 | Bloco 20, Eixo B (B6): mapas nativos completos — 4 tipos, chave `verso` por conteúdo, snapshot de iteração, ordem de inserção determinística, paridade de stdout |
| 218 | Bloco 20, Eixo B (B7): leques com carga nativos — handles `[tag][n][cap][cargas]`, AST recursiva nativa; **avaliador da Fase 210 executa nativo com paridade** |
| 219 | Bloco 20, Eixo B (B8): família texto completa nativa — 17 operações + `formatar_verso` por aridade + interpolação; **o compilador de brinquedo da Fase 211 executa como ELF com paridade** |
| 220 | Bloco 20, Eixo B (B9): arquivo/caminho/tempo/acaso nativos — modelo de handles do interpretador, mesmo algoritmo civil de datas, **mesmo LCG (paridade de sementes)** |
| 221 | Bloco 20, Eixo B (B10): ambiente/processo nativos — argv/env consumindo o `argc`/`argv` da B1, subprocessos completos; **paridade verificada com argumentos reais** |
| 222 | Bloco 20, Eixo B (B11): marco de paridade e fechamento do eixo — suíte automatizada executa exemplos versionados compatíveis nos dois modos, comparando stdout e exit; **Eixo B encerrado** |
| 223 | Bloco 20, Eixo A: error handling estruturado inicial — `tentar` com braços `sucesso`/`falha` sobre leques de resultado declarados pelo usuário, com paridade interpretador × nativo |
| 224 | Bloco 20, Eixo A: propagação explícita — `propagar expr como Resultado.Ok(v) senao Resultado.Erro(e);` retorna falha antecipadamente e mantém lowering nativo |

Histórico completo por fase: `docs/history/phases/`.

## 3. Rodada atual
- **Fase 224 — Eixo A, item 5 da Faixa 1: propagação explícita com `propagar`**.
- A Fase 223 abriu `tentar resultado { sucesso Leque.Ok(v) { ... } falha Leque.Erro(e) { ... } }`; a Fase 224 adiciona `propagar expr como Leque.Ok(v) senao Leque.Erro(e);`, validado no parser e abaixado para retorno antecipado sobre a infraestrutura existente de leques/controle, com execução no interpretador e no backend nativo.
- Suíte B11: manifesto explícito em `tests/backend_nativo_tests.rs` com os exemplos versionados compatíveis do Eixo B (`fase212`–`fase221`), o caso com `argv` real da Fase 221 e os marcos self-hosting compatíveis (`fase209`, `fase210`, `fase211`).
- Critério de pronto cumprido: cada caso roda no interpretador e como ELF nativo gerado por `pink build --nativo`; o stdout do programa é comparado byte a byte e o retorno de `principal` no interpretador é comparado ao exit code nativo.
- Fechamento: **Eixo B encerrado**; o backend `.s` próprio + runtime `pinker_rt` passam a ser a base obrigatória para novas fases de linguagem.
- Limites honestos mantidos: `ouvir` interativo, ordem de iteração de mapa multi-chave e exemplos dependentes de argv/binários auxiliares fora do manifesto controlado não viram critério global.
- `make ci` passa integralmente.

## 4. Limites canônicos ativos

| Recorte | Limite |
|---|---|
| 18.6 (Fases 186–189, 207) | `trazer familia;` funciona para as 7 famílias públicas; `trazer familia.simbolo;` não suportado; domínios provisórios (`colecao`, `formato`) não importáveis; sem modo estrito |
| Fechamento do Bloco 18 | Sem resolução qualificada (`familia.intrinseca`), sem importação seletiva, sem modo estrito, sem reorganização do engine |
| Fases 190–206 | Sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação monomorphizada; sem coleções heterogêneas |
| Fases 208–210 (`leque`/`encaixe`) | Cargas: `bombom`, `verso` ou leque declarado (sem `ninho`/coleções como carga); sem guards, padrões aninhados ou encaixe-expressão; igualdade direta e `virar` rejeitados para leque com carga; sem discriminante customizado; sem `bombom -> leque`; handles sem liberação (consistente com coleções); nome de leque tem precedência sobre variável homônima em posição de base `X.Y` |
| Fase 211 (`lista<T>`) | T = leque declarado (além de `bombom`/`verso` legados); `mapa<K,V>` genérico fora; funções genéricas de usuário fora; generics em `leque`/`ninho` fora; `lista_criar()` só como init de `nova` anotada |
| Bloco 20 | Nenhum item das faixas está entregue por constar na trilha; entrega exige fase numerada com validação objetiva |
| Geral | Compatibilidade global legada preservada integralmente |

## 5. Próximo passo
- Estrutura do Bloco 20 formalizada em dois eixos (Doc-41) e novo padrão pós-Eixo B registrado na Doc-42: **Eixo A — linguagem** retoma com implementações adultas orientadas por `docs/expandir.md`, não por “mínimo” automático; **Eixo B — backend nativo** está encerrado. Ordem vigente agora: A (itens 1–3 ✓) → B (integral ✓) → A (itens 5 → 6 → 4).
- Próxima fase: continuar a expansão do Eixo A sem recorte mínimo automático. No item 5, ainda faltam biblioteca padrão de resultado, extração nomeada do valor propagado e integração com diagnósticos do compilador; depois seguem itens 6 (closures) e 4 (traits), mantendo lowering nativo obrigatório.
- Escada completa do eixo encerrado (B1 ✓ ... B11 ✓) em `docs/roadmap/blocos/bloco_20.md`.
- Depois do item 5: itens 6 (**closures**) e 4 (**traits**) do Eixo A, mantendo a regra de que toda fase de linguagem entrega o lowering nativo junto.

## 6. Arquitetura documental ativa
- `roadmap.md` = ordem ativa.
- `roadmap/indice.md` = hub de navegação por blocos.
- `roadmap/blocos/*.md` = detalhe estrutural por bloco.
- `history.md` = ponteiro canônico curto do histórico.
- `history/indice.md` = hub histórico principal.
- `history/*/indice.md` = roteadores locais por categoria.
- `history/*/*.md` = shards factuais do histórico.
- `handoff_codex.md` = estado operacional unificado (este arquivo).
- `atlas.md` = navegação mestre.
- `ponte_engine_rosa.md` = mediação estável Engine ↔ Rosa.
- `inventario_intrinsecas.md` = inventário canônico de intrínsecas.
- `expandir.md` = referência de expansão para elevar implementações históricas mínimas/conservadoras.
- `docs/phases.md` está ausente no workspace atual; referências legadas devem apontar para `docs/history.md` e shards em `docs/history/`.

## 7. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.

## 8. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP local (`pinker_mcp`) existe novamente como servidor stdio zero-dependency e restrito ao projeto. A superfície atual é apenas de leitura/análise (`pinker_checar`, `pinker_tokens`, `pinker_render`); não expõe execução `--run`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
