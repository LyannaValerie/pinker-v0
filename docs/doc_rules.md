# Regras de documentação da Pinker — referência obrigatória para agentes de IA

- **Classe:** Engine
- **Papel:** regra
- **Status:** ativo

## 1. Arquitetura documental oficial

A documentação da Pinker segue arquitetura dual:

- **Engine**: estado factual/operacional.
- **Rosa**: identidade lexical/estética/visionária.
- **Ponte**: documentos que conectam ambos sem confundir presente e aspiração.

Arquivo mestre de navegação: `docs/atlas.md`.
Sistema histórico canônico: `docs/history.md` -> `docs/history/indice.md` -> índices locais por categoria -> shards em `docs/history/`. `docs/phases.md` não existe mais no workspace atual; referências legadas devem apontar para esse sistema histórico shardado.
Arquitetura do roadmap: `docs/roadmap.md` -> `docs/roadmap/indice.md` -> `docs/roadmap/blocos/bloco_XX.md`.

## 2. Quando atualizar cada documento

| Evento | Documento(s) | O que registrar |
|---|---|---|
| fase funcional concluída | `docs/history/phases/*.md`, `docs/handoff_codex.md` | entrada de fase, estado corrente e handoff curto |
| hotfix extraordinário | `docs/history/hotfixes/*.md`, `docs/handoff_codex.md` | entrada HF dedicada e impacto operacional |
| rodada documental | `docs/history/documentation/*.md`, `docs/handoff_codex.md` | entrada Doc dedicada e limites da rodada |
| rodada paralela de implementação | `docs/history/parallel_phases/*.md`, `docs/handoff_codex.md` | entrada Paralela dedicada |
| abertura/fechamento de bloco | `docs/roadmap.md`, `docs/roadmap/indice.md`, `docs/roadmap/blocos/bloco_XX.md`, `docs/handoff_codex.md`, `docs/history/phases/*.md` ou `docs/history/documentation/*.md` | transição de bloco, shard estrutural e justificativa |
| decisão lexical relevante | `docs/vocabulario.md`, `docs/history/phases/*.md` ou `docs/history/documentation/*.md` | aceitação/rejeição/provisório + referência histórica |
| criação/mudança estrutural de docs | `docs/atlas.md`, `README.md`, `docs/history/documentation/*.md`, `docs/handoff_codex.md` | navegação atualizada e migração registrada |
| criação/atualização de referência de expansão | `docs/expandir.md`, `docs/atlas.md`, `docs/history/documentation/*.md`, `docs/handoff_codex.md` | escopo, critérios e integração documental da referência |
| intrínseca nova adicionada | `docs/inventario_intrinsecas.md`, `docs/handoff_codex.md`, `docs/history/phases/*.md` | entrada no inventário, fase histórica |
| exemplo/teste novo adicionado | `docs/examples_index.md`, `docs/history/phases/*.md` | entrada no índice e fase histórica |

## 3. Formato por documento

- `docs/atlas.md`: navegação mestre e classes documentais.
- `roadmap.md`: topo executivo da trilha ativa oficial e da ordem de execução real.
- `docs/roadmap/indice.md`: hub curto de navegação dos blocos do roadmap.
- `docs/roadmap/blocos/*.md`: detalhe estrutural por bloco; resumem estado factual sem virar crônica histórica.
- `future.md`: inventário técnico amplo; não impõe sequência ativa.
- `rosa.md`: visão identitária canônica da linguagem.
- `parallel.md`: acervo visionário de apoio (não backlog).
- `ponte_engine_rosa.md`: conexão explícita entre factual e visão.
- `docs/history.md`: ponteiro formal do sistema histórico.
- `docs/history/indice.md`: hub principal de navegação histórica.
- `docs/history/*/indice.md`: índices roteadores por categoria.
- `docs/history/*/*.md`: shards com o conteúdo factual das entradas.
- `docs/expandir.md`: referência operacional para planejar expansões de implementações históricas mínimas/conservadoras, sem virar roadmap paralelo.
- `handoff_codex.md`: estado operacional unificado (estado corrente, handoff da rodada, limites, restrições, arquitetura documental).
- `agent_state.md`: redirecionamento para `handoff_codex.md`.
- `vocabulario.md`: arquitetura lexical canônica.
- `docs/phases.md`: ausente no workspace atual; não criar novo arquivo de compatibilidade por inércia. Use `docs/history.md` e shards em `docs/history/`.

## 4. Classificação obrigatória para papéis estruturais

Documentos novos (ou antigos com papel estrutural novo) devem declarar no topo, quando fizer sentido:

- **Classe:** Engine / Rosa / Ponte
- **Papel:** histórico / estado / visão / léxico / navegação / regra / referência / híbrido
- **Status:** ativo / operacional / referência / aspiracional / híbrido

## 5. O que NÃO colocar em cada documento

- `agent_state.md` é redirecionamento; não recebe conteúdo novo.
- `docs/roadmap.md` não vira segundo `history.md`.
- `docs/roadmap/indice.md` não vira réplica dos shards.
- `docs/roadmap/blocos/*.md` não viram crônica fase por fase.
- `docs/history.md` não volta a carregar a crônica inteira.
- `docs/history/indice.md` e os índices locais não viram réplica dos shards.
- `handoff_codex.md` não duplica crônica completa.
- `future.md` não vira roadmap.
- `rosa.md` não declara funcionalidades como implementadas sem evidência Engine.

## 6. Precedência documental

1. Código mergeado.
2. `roadmap.md` (ordem ativa).
3. `handoff_codex.md` (estado operacional unificado).
4. sistema histórico canônico (`docs/history.md` -> `docs/history/indice.md` -> shards em `docs/history/`).
5. `future.md` (inventário técnico).
6. `rosa.md` + `vocabulario.md` + `parallel.md` (identidade e visão).
7. `ponte_engine_rosa.md` e `atlas.md` (mediação e navegação).

## 7. Tom e linguagem

- português;
- objetivo;
- autocontido;
- sem inflar retórica;
- sem duplicação desnecessária.

## 8. Checklist obrigatória por tipo de fase

### Fase funcional (mudança em `src/`, `tests/` ou `examples/`)

- [ ] entrada em `docs/history/phases/*.md` (formato: número, título, bullets factuais)
- [ ] `docs/handoff_codex.md` atualizado (estado corrente, rodada, limites)
- [ ] `docs/examples_index.md` atualizado se exemplo novo foi criado
- [ ] `docs/inventario_intrinsecas.md` atualizado se intrínseca nova foi adicionada
- [ ] `docs/future.md` atualizado se funcionalidade listada lá foi entregue
- [ ] `make ci` passa integralmente

### Fase documental (sem mudança funcional)

- [ ] entrada em `docs/history/documentation/*.md`
- [ ] `docs/handoff_codex.md` atualizado
- [ ] `docs/roadmap.md` e/ou `docs/roadmap/blocos/*.md` atualizados se bloco abriu/fechou

### Hotfix

- [ ] entrada em `docs/history/hotfixes/*.md`
- [ ] `docs/handoff_codex.md` atualizado

### Regra geral

Toda fase que altera a superfície pública da linguagem (nova keyword, novo tipo, nova intrínseca, novo construto sintático) deve atualizar também `MANUAL.md` e `README.md` quando a mudança afetar a apresentação pública do projeto.

## 9. Arquivos de saúde comunitária

Arquivos de saúde comunitária são superfícies operacionais de colaboração externa. Esta classe inclui somente:

- `CONTRIBUTING.md`;
- `CODE_OF_CONDUCT.md`;
- `SECURITY.md`;
- `GOVERNANCE.md`;
- `SUPPORT.md`;
- `.github/ISSUE_TEMPLATE/**`;
- `.github/DISCUSSION_TEMPLATE/**`;
- `.github/pull_request_template.md`.

Uma mudança limitada a essa classe não exige automaticamente fase documental, shard histórico, atualização de handoff, roadmap ou Atlas. A exceção só vale quando a mudança não altera comportamento da linguagem, roadmap técnico, estado de implementação, precedência factual, ownership, autoridade de merge, obrigações de segurança, direção do projeto ou fronteiras Engine/Rosa.

A exceção não vale quando a mudança altera materialmente autoridade de governança, política de segurança, elegibilidade de contribuição, política de merge, direção do projeto, estado técnico canônico ou estrutura de um território documental. Nesses casos, a mudança permanece estrutural e deve atualizar os registros canônicos correspondentes ao seu escopo real.

### Transição única autorizada

A fundação comunitária registrada na Doc-49 é uma exceção única autorizada pelo Founder sob a regra anterior: ela cria estas superfícies e institui a exceção permanente estreita. Para tornar a transição auditável, atualiza `docs/doc_rules.md`, `docs/atlas.md`, `docs/handoff_codex.md`, um shard histórico documental e seu índice, sem mudar roadmap, linguagem, precedência factual ou autoridade final do Founder.
