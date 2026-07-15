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
| Fase funcional mais recente | **222** — Eixo B: marco de paridade e fechamento do eixo (B11); **Eixo B ENCERRADO** |
| Rodada documental mais recente | **Doc-41** — formalização dos dois eixos do Bloco 20 (A — linguagem; B — backend nativo) |
| Bloco ativo | **20** — expansão funcional rumo a SO e self-hosting (Eixo A retomado; Eixo B encerrado) |
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
| 222 | Bloco 20, Eixo B (B11): **suíte de paridade** (155 exemplos interpretador × nativo); **Eixo B ENCERRADO** — runtime `pinker_rt` ~90 funções ABI-C |

Histórico completo por fase: `docs/history/phases/`.

## 3. Rodada atual
- **Fase 222 — Eixo B, fase B11: marco de paridade e fechamento do eixo**.
- Suíte automatizada `suite_de_paridade_interpretador_nativo` descobre cada exemplo versionado, roda nos dois modos e exige stdout + exit idênticos (retorno de `principal` mod 256); 155 exemplos com paridade verificada no CI, piso de 150 contra regressão.
- Categorias fora da paridade documentadas no teste: tempo ao vivo, ordem de iteração de mapa (HashMap não determinístico no interpretador), memória virtual (ponteiros com endereços literais).
- **Eixo B encerrado**: Fases 212–222 levaram toda a superfície da linguagem ao executável nativo; substituiu os testes de paridade individuais das Fases 215–221.
- `make ci` passa integralmente.

## 4. Limites canônicos ativos

| Recorte | Limite |
|---|---|
| 18.6 (Fases 186–189, 207) | `trazer familia;` funciona para as 7 famílias públicas; `trazer familia.simbolo;` não suportado; domínios provisórios (`colecao`, `formato`) não importáveis; sem modo estrito |
| Fechamento do Bloco 18 | Sem resolução qualificada (`familia.intrinseca`), sem importação seletiva, sem modo estrito, sem reorganização do engine |
| Fases 190–206 | Sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação monomorphizada; sem coleções heterogêneas |
| Fases 208–210 (`leque`/`encaixe`) | Cargas: `bombom`, `verso` ou leque declarado (sem `ninho`/coleções como carga); sem guards, padrões aninhados ou encaixe-expressão; igualdade direta e `virar` rejeitados para leque com carga; sem discriminante customizado; sem `bombom -> leque`; handles sem liberação (consistente com coleções); nome de leque tem precedência sobre variável homônima em posição de base `X.Y` |
| Fase 211 (`lista<T>`) | T = leque declarado (além de `bombom`/`verso` legados); `mapa<K,V>` genérico fora; funções genéricas de usuário fora; generics em `leque`/`ninho` fora; `lista_criar()` só como init de `nova` anotada |
| Eixo B (Fases 212–222) | Encerrado; backend nativo cobre a superfície da linguagem exceto ponteiros com endereços reais (Faixa 7 do Eixo A); alvo: x86-64 Linux; runtime `pinker_rt` sem liberação explícita de memória (consistente com o interpretador) |
| Bloco 20 | Nenhum item das faixas está entregue por constar na trilha; entrega exige fase numerada com validação objetiva |
| Geral | Compatibilidade global legada preservada integralmente |

## 5. Próximo passo
- **Eixo B encerrado (Fase 222).** A trilha ativa volta ao **Eixo A**, Faixa 1, na ordem decidida: item **5 (error handling estruturado)** → item **6 (closures)** → item **4 (traits)**.
- **Regra permanente agora em vigor:** toda fase de linguagem nova entrega o **lowering nativo junto** (runtime `pinker_rt` + mapa em `runtime_intrinsic_symbol` no backend), sob pena de reabrir o débito que o Eixo B saldou. A suíte de paridade (`suite_de_paridade_interpretador_nativo`) valida isso automaticamente para cada exemplo novo.
- Próxima fase (prevista 223): item 5 — error handling. Keywords candidatas no vocabulário provisório: `amparo`/`tropeco` (ou Result via `leque`, aproveitando `encaixe`). Considerar que o interpretador aborta em erro de runtime hoje; a fase precisa de um mecanismo de propagação e captura sem abortar.
- Trilha completa: `docs/roadmap/blocos/bloco_20.md`.

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
