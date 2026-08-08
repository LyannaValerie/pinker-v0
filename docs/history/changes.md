---
pinker-doc: 1
id: history.changes
domain: history
kind: index
status: active
parent: history
audience:
  - human
  - agent
related:
  - history
---

# Crônica mecânica de mudanças (pós-marco)

- **Classe:** Engine
- **Papel:** projeção consultável do ledger
- **Status:** ativo

Esta é a **visão humana do ledger** de mudanças da Trama Pinker: uma projeção
mecânica e crescente dos manifestos versionados em `.pinker/changes/`. A crônica
histórica antiga permanece intocada em `phases/`, `hotfixes/` e afins; aqui só
entram mudanças posteriores ao marco #330.

<!-- @pinker-doc:start
id: history.recent
tags: [historico, recente, mudancas, ledger]
aliases:
  - historico recente
  - onde esta o historico recente
  - mudancas recentes
summary: Onde encontrar as mudanças recentes projetadas dos manifestos versionados.
-->
## Mudanças recentes

As mudanças recentes são projetadas mecanicamente da fonte estrutural
(`.pinker/changes/pr-N.yaml`) para a tabela gerada abaixo. Para inspecionar um
manifesto específico, abra o arquivo correspondente; para navegar por comando,
use `pink doc mostrar history.recent`.
<!-- @pinker-doc:end history.recent -->

## Ledger projetado

Conteúdo abaixo é **propriedade da ferramenta** (projeção `history`); não edite à
mão. Regenere com `pink doc sincronizar`.

<!-- @pinker-generated:start change.history -->
| PR | Tipo | Fase | Bloco | Título | Status |
|---|---|---|---|---|---|
| #378 | documentation | — | — | Simplifica o caminho de contribuição externa | completed |
| #382 | parallel-phase | — | — | Adiciona mapa agrupado do catálogo de código | completed |
| #410 | phase | 246 | 20 | Alocação e liberação explícitas de memória | completed |
| #411 | phase | 248 | 20 | Uniões estruturais tagged | completed |
| #412 | hotfix | — | 20 | Endurecimento do runtime nativo pós-PR | completed |
| #418 | parallel-phase | — | — | Aceita listas como cargas tipadas de variantes | completed |
| #419 | documentation | — | 20 | Ativa janela auxiliar de infraestrutura pré-Eixo A | completed |
| #420 | parallel-phase | — | 20 | Normaliza descoberta, ajuda e versão da CLI | completed |
| #421 | hotfix | — | 20 | Contenção da memória pública e eliminação da materialização ansiosa | completed |
| #422 | hotfix | — | 20 | Contenção do host para execuções nativas | completed |
| #424 | hotfix | — | 20 | Integridade da fixture de testes: publicação segura de executáveis | completed |
| #425 | hotfix | — | 20 | Fecha a janela de interrupção ao iniciar a campanha do runner de estabilidade | completed |
| #426 | parallel-phase | — | 20 | Adiciona o contrato somente leitura dos snapshots históricos de projeção | completed |
| #428 | parallel-phase | — | 20 | Adiciona o núcleo determinístico e somente leitura das automações internas | completed |
| #429 | parallel-phase | — | 20 | Adiciona descoberta de root, política de paths e aplicação local atômica | completed |
| #432 | parallel-phase | — | 20 | Completa o lifecycle de CANDIDATE e a CLI de projeções no Stage E | completed |
<!-- @pinker-generated:end change.history -->
