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
| Fase funcional mais recente | **207** — importação mínima de `arquivo`, `caminho` e `processo` (conclusão de 18.6) |
| Rodada documental mais recente | **Doc-39** — fechamento do Bloco 18 e abertura do Bloco 20 |
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

Histórico completo por fase: `docs/history/phases/`.

## 3. Rodada atual
- **Fase 207 — conclusão de 18.6, fechamento do Bloco 18 e abertura do Bloco 20**.
- Funcional: `IMPORTABLE_BUILTIN_FAMILIES` em `src/semantic.rs` ampliado para as 7 famílias públicas; loader inalterado; exemplo `examples/fase207_trazer_arquivo_caminho_processo_valido.pink`; 7 testes semânticos e 2 testes CLI novos.
- Documental: Bloco 18 encerrado por suficiência conservadora (18.7/18.8 no recorte mínimo; 18.9/18.10 declinados; 18.11 executado); shard histórico `201a250.md` criado com as Fases 201–207; **Bloco 20 aberto como trilha ativa** com 11 faixas priorizadas em `docs/roadmap/blocos/bloco_20.md`.
- `make ci` passa integralmente.

## 4. Limites canônicos ativos

| Recorte | Limite |
|---|---|
| 18.6 (Fases 186–189, 207) | `trazer familia;` funciona para as 7 famílias públicas; `trazer familia.simbolo;` não suportado; domínios provisórios (`colecao`, `formato`) não importáveis; sem modo estrito |
| Fechamento do Bloco 18 | Sem resolução qualificada (`familia.intrinseca`), sem importação seletiva, sem modo estrito, sem reorganização do engine |
| Fases 190–206 | Sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação monomorphizada; sem coleções heterogêneas |
| Bloco 20 | Nenhum item das faixas está entregue por constar na trilha; entrega exige fase numerada com validação objetiva |
| Geral | Compatibilidade global legada preservada integralmente |

## 5. Próximo passo
- Iniciar a **Faixa 1** do Bloco 20, começando pelo item 1: **enums / tipos algébricos** (pré-requisito de pattern matching e da direção self-hosting).
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
