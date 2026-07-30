# Roadmap macro da Pinker (trilha oficial ativa)

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

`docs/roadmap.md` é o topo executivo da ordem ativa oficial da Pinker v0.

## Papel deste arquivo

- preservar a trilha única oficialmente ativa;
- deixar inequívoco qual bloco está em curso;
- apontar para o índice e para os shards estruturais do roadmap;
- evitar que o roadmap volte a funcionar como crônica factual longa.

## Ordem ativa oficial

- A Pinker segue uma trilha única de execução.
- O bloco oficialmente ativo é o **Bloco 20 — expansão funcional rumo a SO e self-hosting (trilha por faixas)**, aberto na **Fase 207**.
- O bloco mais recentemente encerrado é o **Bloco 18 — core nobre e bibliotecas temáticas**, encerrado por suficiência conservadora na **Fase 207**.
- O **Bloco 19 — superfície Pinker** permanece candidato futuro, não ativo; a ativação do Bloco 20 antes dele é decisão estratégica explícita, subordinada aos dois propósitos de longo prazo do projeto.
- A frente pausada oficial permanece o **editor/TUI da Pinker**, aberta na **Fase 136** e não abandonada.

## Retomada operacional do Eixo A

Em 28 de julho de 2026, decisão humana explícita da Founder encerrou
antecipadamente a janela de estabilização estrutural e reativou a expansão
funcional do **Eixo A — linguagem**. O Bloco 20 permanece estruturalmente ativo;
a Fase 244 foi a última fase anterior à retomada. As Fases 245 e 246 foram
entregues consecutivamente; as Fases 247 e 248 entregaram os itens 14 e 15.

Os objetivos estruturais da janela encerrada não são declarados concluídos.
Modularização ampla, reorganização documental ampla e bughunting amplo foram
adiados. Os artefatos de auditoria local permanecem evidência histórica,
preservada pelo Git e pela PR #408.

Uma futura modularização deve priorizar inicialmente arquivos com mais de 5.000
linhas, sem tratar tamanho como critério único. Arquivos menores só entram nesse
trabalho quando houver justificativa arquitetural material.

## Bloco ativo atual

**Bloco 20 — expansão funcional rumo a SO e self-hosting (trilha por faixas)**

**Tese estratégica**
Expandir a linguagem na direção dos dois propósitos de longo prazo: gerar um sistema operacional usando apenas Pinker e tornar a Pinker capaz de escrever o próprio código (self-hosting).

**Estado atual**
O bloco executa em **dois eixos** (Doc-41): **Eixo A — linguagem** (11 faixas, 52 itens inventariados frente a C, C#, C++, Python, TypeScript e Shell) e **Eixo B — paridade real do backend nativo** (B1–B11, Fases 212–222, encerrado). Do Eixo A, os itens 1 (enums), 2 (pattern matching) e 3 (generics) da Faixa 1 foram entregues e depois expandidos em fases numeradas: item 3 inclui `lista<T>`, `mapa<K,V>`, funções genéricas explícitas até a Fase 236 e leques genéricos explícitos na Fase 240. A Faixa 2 nasceu concluída pelo fechamento do Bloco 18. O Eixo B foi executado nas Fases 212–222 e está encerrado; o item 5 (error handling) foi retomado nas Fases 223–224, 231, 237, 240 e 241 (esta última predeclarando a biblioteca padrão `Resultado<T,E>` sem declaração manual), o item 6 (closures) foi iniciado na Fase 225, expandido nas Fases 238–239, ganhou valores de função materializados e chamada indireta real na Fase 242 e captura imutável por valor na Fase 243, e o item 4 (traits/interfaces) foi iniciado nas Fases 226–234 e concluído na Fase 244 com objetos de trato, vtables e despacho dinâmico nativo, sempre com lowering nativo obrigatório e com o padrão de expansão registrado em `docs/expandir.md`.

Os itens 12–14 da Faixa 3 foram entregues nas Fases 245–247 com ponteiros crus
de função, memória explícita e assembly inline. O item 15 da Faixa 4 foi
entregue na Fase 248 com uniões estruturais tagged; o item 16 não foi iniciado.
A revisão humana da PR #411 mantém a Fase 248 sob correção: `encaixe` já é um
construto tipado com tags exclusivamente do registry canônico, mas a identidade
nominal na injeção e os payloads multi-palavra continuam abertos.

O contrato de ambientes da Fase 243 usa hoje uma palavra por captura (`quantidade * palavra`, com overflow verificado). Uma futura representação multi-palavra deverá alocar o tamanho final alinhado do layout, derivado do tamanho, alinhamento, offset e padding de cada captura e do alinhamento final, com overflow verificado em cada passo; soma simples de tamanhos e teste apenas funcional não atendem ao gate direto de underallocation.

A **Doc-46** formaliza a trilha transversal bare-metal e bootstrap como convergência adulta entre as capacidades do Eixo A e uma cadeia freestanding verificável. Ela adota explicitamente o padrão anti-mínimo: cada fase deve fechar um subproblema operacional com superfície, semântica, backend/runtime, diagnósticos, testes, exemplo e documentação; stubs e provas de conceito isoladas não contam como entrega.

**Escada macro**
- Eixo A, Faixa 1 — funcionalidades de alta dificuldade (itens 1–3 entregues; 5, 6 e 4 após o Eixo B);
- **Eixo B — paridade real do backend nativo** (runtime próprio + lowering completo da superfície atual; B1–B11 entregues nas Fases 212–222);
- Eixo A, Faixa 3 — ponteiros de função, alocador de memória, inline assembly real;
- **trilha bare-metal e bootstrap** — toolchain freestanding, runtime autônomo, contrato de boot/hardware e produto de build validado;
- Eixo A, Faixas 4–6 — sistema de tipos, funções e controle de fluxo;
- Eixo A, Faixas 7–9 — baixo nível, metaprogramação, módulos e build;
- Eixo A, Faixas 10–11 — concorrência, SO, I/O e rede.

**Detalhe estrutural**
- `docs/roadmap/blocos/bloco_20.md`
- `docs/roadmap/bare_metal_bootstrap.md`

## Relação com os demais documentos

- `docs/roadmap.md` define a ordem ativa.
- `docs/roadmap/indice.md` organiza a navegação curta por blocos.
- `docs/roadmap/blocos/bloco_XX.md` guardam o detalhe estrutural de cada bloco.
- `docs/roadmap/bare_metal_bootstrap.md` detalha a convergência freestanding do Bloco 20 e seus critérios anti-mínimo sem declarar implementação.
- `docs/history.md` e `docs/history/` preservam a crônica factual detalhada.
- `docs/future.md` continua sendo inventário técnico e não dita a ordem ativa.

## Navegação

- Hub do roadmap: `docs/roadmap/indice.md`
- Bloco ativo atual: `docs/roadmap/blocos/bloco_20.md`
- Trilha bare-metal do bloco ativo: `docs/roadmap/bare_metal_bootstrap.md`
- Bloco recém-encerrado: `docs/roadmap/blocos/bloco_18.md`
- Candidato futuro não ativo: `docs/roadmap/blocos/bloco_19.md`
