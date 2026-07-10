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
| Fase funcional mais recente | **206** — tipo `mapa<bombom,verso>` |
| Rodada documental mais recente | **Doc-38** — restauração do mapa macro do Bloco 18 |
| Bloco ativo | **18** — core nobre e bibliotecas temáticas |
| Último bloco encerrado | **17** — forma visual e superfície documental (Fase 176) |
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

### Fases recentes do Bloco 18

| Fases | Contribuição |
|---|---|
| 180–185 | Abertura documental: inventário de intrínsecas, famílias públicas, `tempo` como exemplar, domínios internos, resolução qualificada (preparação) |
| 186–189 | 18.6: `trazer tempo;`, `trazer ambiente;`, `trazer acaso;`, `trazer texto;` |
| 190–202 | Ergonomia: comentários de bloco, escape sequences, operadores compostos, intrínsecas utilitárias, literais negativos, multiline strings, `repetir...até`, `para...de...até`, `eterno` verso, ternário, `escolha/caso`, retorno implícito, interpolação |
| 203–206 | Coleções: `lista<verso>`, `mapa<verso,verso>`, `mapa<bombom,bombom>`, `mapa<bombom,verso>` |

Histórico completo por fase: `docs/history/phases/`.

## 3. Rodada atual
- **Fases 190–206 — ergonomia, expressividade e expansão de coleções**.
- Expansão funcional ampla cobrindo ergonomia de linguagem (Fases 190–202) e novos tipos de coleção (Fases 203–206), fora do escopo formal do Bloco 18 mas compatível com o domínio provisório `colecao`.

### Resultado operacional da rodada
- `src/lexer.rs`: comentários de bloco (`/* */`), sequências de escape em verso (`\n`, `\t`, `\\`, `\"`), strings multiline.
- `src/parser.rs`: operadores compostos de atribuição (`+=`, `-=`, `*=`, `/=`, `%=`), `repetir ... até`, `para ... de ... até`, operador ternário (`? :`), `escolha/caso/padrao`, retorno implícito, interpolação de verso (`"texto {expr} texto"`), desugaring de `para cada` para novos tipos de coleção.
- `src/semantic.rs`: novas intrínsecas utilitárias (`verso_para_bombom`, `bombom_para_verso`, `dormir`, `afirmar`, `copiar_arquivo`, `renomear_arquivo`, `aleatorio_entre`), suporte semântico completo para `lista<verso>`, `mapa<verso,verso>`, `mapa<bombom,bombom>`, `mapa<bombom,verso>` e intrínsecas de iteração associadas, `eterno` para verso.
- `src/ir.rs`, `src/ir_validate.rs`, `src/cfg_ir_validate.rs`, `src/instr_select_validate.rs`, `src/abstract_machine_validate.rs`, `src/layout.rs`: novos tipos `ListVerso`, `MapVersoVerso`, `MapBombomBombom`, `MapBombomVerso` e function sigs correspondentes em todos os estágios de validação.
- `src/interpreter.rs`: implementação runtime de todas as novas intrínsecas e tipos de coleção, incluindo iteradores internos para `para cada`.
- `src/main.rs`, `src/repl.rs`, `src/ast.rs`: suporte aos novos tipos em display e AST.
- Exemplos versionados: `examples/fase190_*` a `examples/fase206_*` (17 exemplos).
- `make ci` passa integralmente.

## 4. Limites canônicos ativos

| Recorte | Limite |
|---|---|
| 18.6 (Fases 186–189) | `trazer tempo;`, `trazer ambiente;`, `trazer acaso;` e `trazer texto;` funcionam; `trazer familia.simbolo;` não suportado; outras famílias não importáveis; sem modo estrito; sem reorganização de engine |
| Fases 190–206 | Sem generics (`lista<T>`, `mapa<K,V>` amplos); cada combinação monomorphizada; sem coleções heterogêneas |
| Bloco 17 | Não abriu sintaxe nova, reforma de keywords, inferência local, `;` opcional, unidade implícita nem redesign de módulos |
| 16.2 | `pipeline_minimo` fora da expansão de `argv1`; sem múltiplos argv gerais, shell implícito, PTY, job control |
| Geral | Compatibilidade global legada preservada integralmente |

## 5. Próximo passo
- Continuar avançando funcionalidades de alta dificuldade restantes ou retomar o eixo 18.6/18.7 do Bloco 18.
- O **Bloco 18** segue como bloco oficialmente ativo.

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
- `inventario_intrinsecas.md` = inventário canônico de intrínsecas (Bloco 18).
- `phases.md` = compatibilidade legada.

## 7. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.
- Não operacionalizar famílias públicas antes da decisão lexical de 18.2.

## 8. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP local (`pinker_mcp`) existe novamente como servidor stdio zero-dependency e restrito ao projeto. A superfície atual é apenas de leitura/análise (`pinker_checar`, `pinker_tokens`, `pinker_render`); não expõe execução `--run`.
- Padrão recomendado: `cargo run --bin pink -- ...`.
