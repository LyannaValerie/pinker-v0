---
pinker-doc: 1
id: development.deterministic-infrastructure-window
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.deterministic-infrastructure-window
related:
  - development.tramas-v1
  - roadmap
---

# Janela auxiliar de infraestrutura determinística

- **Classe:** Engine
- **Papel:** governança operacional
- **Status:** ativo

Este documento transforma em autoridade operacional a decisão humana da
Issue #417. A Issue permanece a origem da decisão; este documento passa a ser a
fonte canônica para agentes, gates e roadmap enquanto a janela estiver ativa.

<!-- @pinker-doc:start
id: development.deterministic-infrastructure-window.current
tags: [desenvolvimento, governanca, infraestrutura, determinismo, eixo-a]
aliases:
  - janela auxiliar
  - janela de infraestrutura deterministica
  - excecao pre eixo a
summary: Estado, autoridade e limites da janela auxiliar autorizada pela Issue #417.
-->
## Estado canônico

```yaml
Issue_417:
  decision: ACTIVE

D1:
  status: COMPLETE
  pr: 418
  merge: 09b7456fd57c2efcf71e54895b938a6a69d77307

janela_auxiliar:
  status: ACTIVE
  completed:
    - Etapa 1 da Issue #414 (PR #420)
    - Issue #385 — núcleo determinístico puro e apply local (PRs #428 e #429)
    - Issue #384 — snapshots, autoridade histórica e lifecycle (PRs #430 e #432)
  extraordinary_interruption: HF-9 — memória pública e contenção do host
  current_delivery: Issue #387 — estado consolidado somente leitura

Eixo_A:
  status: PAUSED_BY_EXPLICIT_HUMAN_DECISION
  next_functional_item_after_window: D2

D2:
  status: NOT_STARTED
```

A pausa do Eixo A não caracteriza defeito nem bloqueio técnico. É uma decisão
explícita e temporária de prioridade, com retorno determinado para D2 após o
encerramento da janela. O Eixo A continua sendo a prioridade funcional soberana:
qualquer conflito material de escopo ou regressão favorece o Eixo A.

O HF-9 foi autorizado diretamente pela mantenedora depois do merge humano da
PR #420. Sua unidade 1 é a memória pública da PR #421; a unidade 2 é a contenção
do host da PR #422. A interrupção extraordinária não ocupa uma das seis
entregas, não altera sua ordem e não inicia D2. Depois do merge humano da
unidade 2, a entrega
corrente volta a ser a segunda capacidade, snapshots e projeções da Issue #384.

## Ativação e efeito

```yaml
activation_condition: PR de governança mergeada sobre main contendo D1
activation_effect: exceção estreita ao portão pré-Eixo A
not_an_effect: revogação geral do portão pós-Trama
```

A condição de ativação exige D1 concluída antes da vigência da janela. D1 foi
concluída pela PR #418 e está contida na `main` pelo merge
`09b7456fd57c2efcf71e54895b938a6a69d77307`. A presença deste documento na
`main`, por merge humano da PR de governança, efetiva a decisão como `ACTIVE`.

A janela não abre Fase, Faixa ou Eixo, não reabre a Trama Pinker V1, não
constitui Trama Nova e não declara `Eixo A — linguagem: COMPLETE`. Todas as
capacidades pós-Trama não enumeradas abaixo continuam adiadas.

## Propósito e autoridade

O propósito é permitir um conjunto fechado de infraestrutura determinística e
observacional que reduza o custo das entregas restantes do Eixo A, sem competir
com a evolução funcional da linguagem. Este documento governa propósito,
estado, capacidades, escrita, ordem, limites por PR, suspensão, encerramento e
retorno a D2.

Em caso de divergência:

1. a Issue #417 preserva a decisão humana de origem;
2. este documento define sua aplicação operacional no repositório;
3. `docs/development/tramas-v1.md`, `AGENTS.md`, roadmap e handoff refletem esta
   autoridade;
4. capacidades não enumeradas não podem ser inferidas, agregadas ou liberadas
   por analogia.

## Capacidades autorizadas

A autorização é exaustiva e contém exatamente seis entregas.

### 1. Etapa 1 da Issue #414

- ajuda;
- versão;
- descoberta;
- códigos de saída;
- nome estável do executável;
- testes de processo.

Somente a Etapa 1 está autorizada; nenhuma etapa posterior da Issue #414 entra
na janela por inferência.

### 2. Issue #384 — snapshots e projeções

- snapshots históricos congelados;
- projeção candidata;
- verificação somente leitura;
- atualização explícita;
- determinismo e idempotência;
- novo inventário sobre a `main` pós-D1;
- PR #397 preservada e não retomada diretamente.

A PR #397 permanece histórica: não deve ser rebaseada, atualizada ou mergeada.
Qualquer implementação de snapshots deve partir de novo inventário. Ideias podem
ser reaproveitadas após inspeção, mas commits não devem ser transplantados
cegamente.

### 3. Recorte observacional da Issue #385

- descoberta canônica do root;
- paths repo-relativos;
- modelo comum de relatório;
- códigos de saída;
- JSON determinístico;
- detecção de drift;
- origem dos dados;
- escrita explícita somente de artefatos derivados.

### 4. Recorte somente leitura da Issue #387

- estado dos catálogos;
- drift;
- projeções;
- checks conhecidos;
- estado observável do agente;
- bloqueadores;
- operações pendentes;
- indisponibilidade explícita;
- origem dos dados.

### 5. Índice derivado de símbolos e `pink nav localizar`

O índice deve ser derivado das autoridades existentes e a operação deve ser
somente leitura, com saída humana e schema público versionado quando houver
contrato estruturado.

### 6. Cobertura de diff somente leitura

A cobertura relaciona diffs a superfícies afetadas por autoridades explícitas,
sem editar fontes, aplicar correções ou apresentar inferência heurística como
certeza.

## Capacidades que continuam adiadas

Permanecem fora da janela e nas listas históricas de trabalho futuro:

- Trama Nova como programa completo;
- Trama Viva;
- TUI;
- novo executor;
- expansão do Rosa Orchestrator;
- expansão ampla do Supervisor;
- orquestração multiagente;
- edição transacional de fontes;
- planos gerais `ADD`, `REMOVE` e `MODIFY` sobre código;
- aplicação automática de patches;
- split ou merge assistido;
- grafo geral de código;
- auditoria integral;
- modularização estrutural ampla;
- novo sistema de publicação;
- merge automático;
- auto-merge;
- qualquer mudança da semântica da linguagem.

## Política de escrita

### Modo padrão

Somente leitura.

### Escrita permitida

Somente artefatos derivados, determinísticos e versionados, mediante comando
explícito. Toda escrita exige:

- plano ou delta anterior;
- destino dentro do root autorizado;
- substituição atômica;
- preservação de predecessores congelados;
- idempotência;
- segunda execução sem delta.

### Escrita proibida

- alteração automática de fontes;
- alteração automática de configuração pessoal;
- operação Git remota implícita;
- merge;
- auto-merge;
- modificação de branches de trabalho de linguagem.

## Ordem das entregas

1. Etapa 1 da Issue #414;
2. snapshots e projeções da Issue #384;
3. núcleo comum observacional da Issue #385;
4. estado consolidado somente leitura da Issue #387;
5. índice derivado de símbolos e `pink nav localizar`;
6. cobertura de diff somente leitura.

A ordem só pode mudar por decisão humana registrada. D2 não começa durante a
janela e permanece o próximo item funcional depois de seu encerramento.

## Limites por PR

- uma PR de implementação da janela ativa por vez;
- uma capacidade coerente por PR;
- exatamente um bloco `pinker-change`;
- nenhum novo requisito obrigatório para D2 sem decisão humana separada;
- nenhuma alteração de AST, IR, CFG, seleção, máquina abstrata, interpretador,
  backend ou runtime, salvo exposição observacional sem mudança funcional;
- schemas públicos versionados;
- geradores executados duas vezes;
- segunda execução sem delta;
- validação em fresh environment;
- testes positivos, negativos e de sensibilidade quando houver contrato
  executável;
- merge exclusivamente humano.

Cada PR deve fechar um único contrato operacional auditável e não pode misturar
duas capacidades autorizadas.

## Critérios de suspensão

Uma entrega deve parar e retornar para decisão humana quando:

- exigir semântica nova de linguagem ou mudança de ABI;
- exigir novo executor;
- exigir escrita automática de fontes;
- duplicar uma autoridade canônica;
- tornar-se requisito para D2 sem autorização separada;
- publicar schema sem versionamento;
- exigir modularização estrutural ampla;
- deixar de caber numa PR coerente;
- produzir regressão ou conflito material com o Eixo A;
- ultrapassar a lista fechada de capacidades autorizadas.

## Encerramento e retorno

O encerramento exige uma PR documental específica, mesmo quando antecipado por
decisão humana. Essa PR deve:

- registrar o inventário final;
- marcar entregas concluídas;
- registrar entregas omitidas ou interrompidas;
- atualizar roadmap e handoff;
- restaurar D2 como prioridade ativa;
- preservar capacidades não autorizadas como adiadas;
- não declarar o Eixo A completo.

A janela pode terminar após as seis entregas, ser encerrada antecipadamente por
decisão humana ou parar diante de limitação arquitetural. Até o merge humano da
PR de encerramento, seu estado canônico permanece `ACTIVE`.
<!-- @pinker-doc:end development.deterministic-infrastructure-window.current -->
