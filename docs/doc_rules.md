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
Sistema histórico canônico: `docs/history.md` -> `docs/history/indice.md` -> índices locais por categoria -> shards em `docs/history/`.
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
- `handoff_codex.md`: estado operacional unificado (estado corrente, handoff da rodada, limites, restrições, arquitetura documental).
- `agent_state.md`: redirecionamento para `handoff_codex.md`.
- `vocabulario.md`: arquitetura lexical canônica.

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
