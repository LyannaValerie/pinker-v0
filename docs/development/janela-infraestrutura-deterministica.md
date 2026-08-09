---
pinker-doc: 1
id: development.deterministic-infrastructure-window
domain: development
kind: reference
status: reference
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
- **Status:** referência histórica

Este documento preserva a autoridade operacional e o inventário final da
decisão humana da Issue #417. A janela foi temporária, concluiu as seis
capacidades autorizadas e não constitui mais exceção ativa ao portão pós-Trama.

<!-- @pinker-doc:start
id: development.deterministic-infrastructure-window.current
tags: [desenvolvimento, governanca, infraestrutura, determinismo, eixo-a, encerramento]
aliases:
  - janela auxiliar
  - janela de infraestrutura deterministica
  - excecao pre eixo a
summary: Inventário final, limites históricos e encerramento da janela auxiliar da Issue #417, com D2 restaurado como próxima prioridade funcional.
-->
## Estado canônico

```yaml
Issue_417:
  decision: CLOSED_COMPLETE

D1:
  status: COMPLETE
  pr: 418
  merge: 09b7456fd57c2efcf71e54895b938a6a69d77307

janela_auxiliar:
  status: CLOSED
  completed:
    - capacidade 1 — descoberta, ajuda e versão (PR #420, merge a22735385fde0a55b2cd1b3a010a9a6063600bda)
    - capacidade 2 — snapshots, autoridade histórica e lifecycle (PRs #426, #430 e #432; merges d5a4008692e612618bf6b3188575e0bbcab3afaa, bcdcaf90635fa302a46e0de41fbe67688bdb2dc3 e 36efa31c96157fbf10ace812eae4906f888a417d)
    - capacidade 3 — núcleo determinístico observacional (PRs #428 e #429; merges bd2107e1c3403cc5b550365dcbd67bafe534f39f e 7be6ca3205741dd674af1cc08d0b6ea06c3ad488)
    - capacidade 4 — estado consolidado somente leitura (Issue #387, PR #433, merge 97ee0226a617439a48e33b73779d7c60caa534e3)
    - capacidade 5 — índice derivado de símbolos e pink nav localizar (Issue #434, PR #435, merge d6cb0febe5cb0ad8419503303aa5959a5ad79af0)
    - capacidade 6 — cobertura de diff somente leitura (Issue #438, PR #439, merge a1c656b7a243c1d5526ea256cc56aa7e65acab54)
  not_executed: []
  extraordinary_interruption: HF-9 — memória pública e contenção do host (PRs #421 e #422)
  current_delivery: none

Eixo_A:
  status: INCOMPLETE
  next_functional_priority: D2

D2:
  status: NEXT_FUNCTIONAL_PRIORITY
  implementation: NOT_STARTED
```

A pausa temporária do Eixo A durante a janela não caracterizou defeito nem
bloqueio técnico. Com o encerramento, D2 volta a ser a próxima prioridade
funcional, ainda sem implementação iniciada. O Eixo A continua incompleto e
soberano; esta transição não o declara `COMPLETE`.

O HF-9 foi autorizado diretamente pela mantenedora depois do merge humano da
PR #420. Sua unidade 1 é a memória pública da PR #421; a unidade 2 é a contenção
do host da PR #422. A interrupção extraordinária não ocupa uma das seis
entregas, não altera sua ordem e não inicia D2. Depois do merge humano da
unidade 2, a entrega corrente voltou a ser a segunda capacidade, snapshots e
projeções da Issue #384.

## Ativação, vigência e encerramento

```yaml
activation_condition: PR de governança mergeada sobre main contendo D1
activation_effect: exceção estreita ao portão pré-Eixo A
not_an_effect: revogação geral do portão pós-Trama
closure_condition: seis capacidades concluídas e PR documental mergeada
closure_effect: D2 restaurado como próxima prioridade funcional
```

A condição de ativação exige D1 concluída antes da vigência da janela. D1 foi
concluída pela PR #418 e está contida na `main` pelo merge
`09b7456fd57c2efcf71e54895b938a6a69d77307`. A presença deste documento na
`main`, por merge humano da PR de governança, efetivou a decisão durante sua
vigência. O merge humano da PR documental de encerramento torna o estado acima
`CLOSED` e fecha a Issue #417 pelo vínculo `Closes #417`.

A janela não abre Fase, Faixa ou Eixo, não reabre a Trama Pinker V1, não
constitui Trama Nova e não declara `Eixo A — linguagem: COMPLETE`. Todas as
capacidades pós-Trama não enumeradas abaixo continuam adiadas.

## Propósito e autoridade

O propósito é permitir um conjunto fechado de infraestrutura determinística e
observacional que reduza o custo das entregas restantes do Eixo A, sem competir
com a evolução funcional da linguagem. Este documento preserva propósito,
estado histórico, capacidades entregues, escrita, ordem, limites por PR,
suspensão, encerramento e retorno a D2.

Em caso de divergência:

1. a Issue #417 preserva a decisão humana de origem;
2. este documento preserva sua aplicação operacional e seu encerramento;
3. `docs/development/tramas-v1.md`, `AGENTS.md`, roadmap e handoff refletem esta
   autoridade;
4. capacidades não enumeradas não podem ser inferidas, agregadas ou liberadas
   por analogia.

## Capacidades autorizadas e concluídas

A autorização foi exaustiva e todas as seis entregas foram concluídas. Nenhuma
entrega autorizada foi omitida ou interrompida.

### 1. Etapa 1 da Issue #414

Concluída pela PR #420.

- ajuda;
- versão;
- descoberta;
- códigos de saída;
- nome estável do executável;
- testes de processo.

Somente a Etapa 1 está autorizada; nenhuma etapa posterior da Issue #414 entra
na janela por inferência.

### 2. Issue #384 — snapshots e projeções

Concluída pelas PRs #426, #430 e #432.

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

Concluído pelas PRs #428 e #429.

- descoberta canônica do root;
- paths repo-relativos;
- modelo comum de relatório;
- códigos de saída;
- JSON determinístico;
- detecção de drift;
- origem dos dados;
- escrita explícita somente de artefatos derivados.

### 4. Recorte somente leitura da Issue #387

Concluído pela PR #433.

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

Concluído pela Issue #434 e PR #435.

O índice deve ser derivado das autoridades existentes e a operação deve ser
somente leitura, com saída humana e schema público versionado quando houver
contrato estruturado.

### 6. Cobertura de diff somente leitura

Concluída pela Issue #438 e PR #439.

A cobertura relaciona diffs a superfícies afetadas por autoridades explícitas,
sem editar fontes, aplicar correções ou apresentar inferência heurística como
certeza.

## Capacidades que continuam adiadas

Permanecem adiadas pelo portão pós-Trama e não foram liberadas pelo
encerramento:

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

## Política de escrita preservada

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

## Ordem executada das entregas

1. Etapa 1 da Issue #414;
2. snapshots e projeções da Issue #384;
3. núcleo comum observacional da Issue #385;
4. estado consolidado somente leitura da Issue #387;
5. índice derivado de símbolos e `pink nav localizar`;
6. cobertura de diff somente leitura.

A ordem não mudou. D2 não começou durante a janela e agora é a próxima
prioridade funcional, sem implementação iniciada por esta PR documental.

## Limites aplicados por PR

- durante a vigência, uma PR de implementação da janela ativa por vez;
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

## Critérios de suspensão preservados

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

O encerramento foi preparado por esta PR documental específica depois da
conclusão das seis capacidades. Ela:

- registra o inventário final;
- marca as seis entregas como concluídas e nenhuma como omitida;
- atualiza roadmap e handoff;
- restaura D2 como próxima prioridade funcional, sem iniciá-lo;
- preserva capacidades não autorizadas como adiadas;
- mantém o Eixo A explicitamente incompleto.

A janela termina pelo merge humano desta PR documental. Na branch da PR, a
Issue #417 permanece aberta; o merge aplica `Closes #417`, publica este estado
`CLOSED` na `main` e mantém D2 como `NEXT_FUNCTIONAL_PRIORITY`.
<!-- @pinker-doc:end development.deterministic-infrastructure-window.current -->
