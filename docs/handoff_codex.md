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
| Fase funcional mais recente | **219** — Eixo B: família texto completa nativa; compilador da Fase 211 executa como ELF (B8) |
| Rodada documental mais recente | **Doc-41** — formalização dos dois eixos do Bloco 20 (A — linguagem; B — backend nativo) |
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

Histórico completo por fase: `docs/history/phases/`.

## 3. Rodada atual
- **Fase 219 — Eixo B, fase B8: família texto completa nativa**.
- Runtime +17 funções de texto usando exatamente as mesmas chamadas std do interpretador (paridade por construção, incl. Unicode); `formatar_verso` via conversão de args `bombom`→verso na IR + wrappers por aridade (0..8) no runtime.
- Locals/params `logica` aceitos no subset externo (último bloqueio do compilador de brinquedo).
- Critério de pronto cumprido: **compilador da Fase 211 nativo com paridade de stdout** (lexer → parser → AST → 42); lexer da Fase 209 idem.
- Cobertura: exemplo fase219 (17 linhas idênticas); 4 testes de paridade/emissão; 2 testes históricos de ninho atualizados; fronteira movida para arquivo (B9).
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
- Estrutura do Bloco 20 formalizada em dois eixos (Doc-41): **Eixo A — linguagem** (faixas) e **Eixo B — backend nativo**. Ordem vigente: A (itens 1–3 ✓) → B (integral, em curso) → A (itens 5 → 6 → 4).
- Próxima fase: **Eixo B, B9 (prevista Fase 220) — arquivo + caminho + tempo + acaso nativos** — intrínsecas de arquivo (handle e caminho) e filesystem sobre a std no runtime; `tempo_unix`/`formatar_tempo_unix`; gerador de acaso (mesmo algoritmo do interpretador para paridade de sementes); exemplos de arquivo executando nativos.
- Escada completa do eixo (B1 ✓ ... B8 ✓, B9–B11) em `docs/roadmap/blocos/bloco_20.md`; regra do eixo: sem recorte mínimo, e B11 fecha com suíte de paridade interpretador × nativo no CI.
- Após o eixo: itens 5 (**error handling**), 6 (**closures**) e 4 (**traits**) do Eixo A, com a regra nova de que toda fase de linguagem entrega o lowering nativo junto.

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
- `phases.md` = compatibilidade legada.

## 7. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.

## 8. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP local (`pinker_mcp`) existe novamente como servidor stdio zero-dependency e restrito ao projeto. A superfície atual é apenas de leitura/análise (`pinker_checar`, `pinker_tokens`, `pinker_render`); não expõe execução `--run`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
