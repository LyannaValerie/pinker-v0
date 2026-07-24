---
pinker-doc: 1
id: development.tramas-v1
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.tramas-policy
related:
  - development.code-navigation
  - development.pink-agent
  - roadmap
---

# Tramas V1

Este documento registra a decisão da Founder de encerrar formalmente a Trama
Pinker V1 e governa a ordem de implementação de quaisquer Tramas futuras.

- A decisão de fechamento é autorizada pela Founder.
- A Trama Pinker V1 está funcionalmente concluída.
- Este documento **não** abre uma fase nem uma nova onda.
- Este documento governa a ordem de implementação das futuras Tramas.

"Pode melhorar" não significa "está incompleta". O fechamento formal reconhece
que a finalidade original foi cumprida, sem impedir evolução posterior.

## Trama Pinker V1

Finalidade original cumprida:

```
portais humanos
+ IDs semânticos
+ âncoras documentais
+ âncoras de código
+ catálogos gerados
+ consultas determinísticas
+ manifestos estruturados
+ verificação de drift
+ CI somente leitura
= memória estrutural navegável
```

Estado:

```
functional_scope: COMPLETE
formal_closure: COMPLETE
waves_8_9: COMPLETE
waves_10_12: NOT_DEFINED
new_functional_waves_required: false
```

As Ondas 10, 11 e 12 nunca receberam definição canônica; nenhuma delas é
necessária para o fechamento. Nenhuma nova onda funcional é exigida.

## Prioridade do Eixo A

- O Eixo A do Bloco 20 retoma imediatamente após o merge do fechamento.
- Futuras Tramas **não** podem entrar no caminho crítico.
- O gate obrigatório é `Eixo A — linguagem: COMPLETE`.
- Correções que bloqueiem o Eixo A continuam permitidas.
- Essas correções **não** reabrem a Trama.

O gate `Eixo A — linguagem: COMPLETE` não deve ser confundido com: conclusão da
Faixa 1; conclusão de uma fase; implementação de `Resultado<T,E>`; ponteiros de
função; alocador; protótipo bare-metal; ou conclusão parcial do Bloco 20. Ele é
satisfeito somente quando o roadmap canônico declarar explicitamente
`Eixo A — linguagem: COMPLETE`.

## Tramas futuras

```
start_before_Eixo_A_complete: false
automatic_authorization: false
require_post_Eixo_A_inventory: true
```

Adiados até depois do Eixo A:

- Trama Nova;
- Trama Viva;
- pós-Trama;
- índice de símbolos;
- `pink nav localizar`;
- cobertura de diff;
- edição transacional;
- split/merge assistido;
- grafo de código;
- auditoria integral;
- expansão do Rosa Orchestrator;
- evolução do Supervisor;
- endurecimento pós-Trama de paridade nativa.

## Capacidades já cobertas

- `pink doc`;
- `pink nav`;
- `pink nav mapa`;
- catálogos determinísticos;
- drift;
- Guardião;
- `pink-agent-v1`;
- allowlists;
- sensibilidade;
- artefatos;
- publicação;
- retomada;
- checks remotos.

## Capacidades reconsideráveis depois do Eixo A

- índice de símbolos;
- símbolo → região;
- `pink nav localizar`;
- `pink nav cobertura-diff`;
- planos `ADD`, `REMOVE`, `MODIFY`;
- aplicação transacional;
- split/merge;
- grafo direto;
- auditoria finita;
- orquestração multiagente mínima.

## Não implementar

- `src/navigation.history.jsonl`;
- `apps/pink_doc`;
- alias `pink guard`;
- backlinks persistidos;
- `updated_by`;
- `class` duplicando domínio;
- migração documental total;
- README em todo diretório;
- Guardião universal heurístico;
- backfill histórico;
- ondas sem finalidade;
- sistema de publicação paralelo;
- novo executor dentro da Trama Nova;
- cérebro integral de Rosa;
- barramento geral de eventos;
- aplicação all-in-one;
- análise interprocedural perfeita.

## PR #397 e snapshots

- A PR #397 é pós-fechamento.
- Não é necessária para fechar a Trama.
- Não conclui a Issue #384.
- Permanece inalterada.
- Não deve ser rebaseada, atualizada ou mergeada antes de `Eixo A: COMPLETE`.
- Qualquer reaproveitamento futuro exige novo inventário sobre a base então
  atual.

As Issues #384–#395 são pós-fechamento ou endurecimento paralelo. Os snapshots
permanecem pós-fechamento.

## Contrato final

```
Trama_Pinker_V1:
  close_now: true
  formal_closure: COMPLETE
  functional_expansion: false

Eixo_A_Bloco_20:
  resume_after_closure: true
  priority_over_future_Tramas: absolute

future_Tramas:
  start_before_Eixo_A_complete: false
  require_new_inventory: true

PR_397:
  relationship: POST_CLOSURE
  mutate_now: false

pink_agent_v1:
  status: FROZEN_OPERATIONAL_INFRASTRUCTURE
```

> "Pode melhorar" não significa "está incompleta".
