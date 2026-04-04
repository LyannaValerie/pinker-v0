# Família exemplar `tempo`

- **Classe:** Ponte
- **Papel:** híbrido
- **Status:** ativo

Este documento formaliza `tempo` como a família exemplar do **Bloco 18 — core nobre e bibliotecas temáticas**.

## 1. Papel canônico no bloco

`tempo` é a família exemplar do bloco porque oferece o menor caso vivo que ainda é tecnicamente útil, lexicalmente promissor e documentalmente auditável.

Ela serve para:

- provar que o Bloco 18 não é apenas taxonomia abstrata;
- registrar uma família pública real sobre superfície já existente;
- separar com clareza o estado legado atual da direção lexical futura;
- preparar fases posteriores sem fingir resolução qualificada já implementada.

## 2. O que pertence hoje à família

Hoje pertencem à família `tempo` exatamente estas intrínsecas públicas já existentes:

- `tempo_unix() -> bombom`
- `formatar_tempo_unix(ts) -> verso`

Não há outros nomes temporais públicos canônicos no workspace atual.

## 3. Superfície pública mínima atual

A superfície mínima atual da família é a própria dupla legada já aceita pelo engine:

- `tempo_unix()` produz o timestamp Unix atual como `bombom`;
- `formatar_tempo_unix(ts)` formata esse timestamp como `verso`.

Leitura canônica:

- a família `tempo` nasce sobre essa superfície já operacional;
- a família ainda não existe como namespace funcional da linguagem;
- os nomes continuam globais e legados na superfície pública real de hoje.

## 4. Compatibilidade lexical preservada

Nesta fase, os nomes aceitos e preservados por compatibilidade são:

- `tempo_unix`
- `formatar_tempo_unix`

Eles permanecem:

- válidos como superfície pública atual;
- canônicos no estado implementado de hoje;
- preservados por honestidade histórica e compatibilidade lexical.

Esta fase não introduz alias novo, renomeação obrigatória, depreciação operacional nem mudança funcional em parser, semântica, runtime ou docs de uso como se a migração já existisse.

## 5. Direção lexical futura, sem compromisso operacional

O horizonte lexical visível da família é reduzir a redundância herdada dos nomes globais quando a Pinker tiver mecanismo próprio para superfície por família.

Direções plausíveis futuras:

- manter a ideia de “Unix” explícita, mas sob forma temática mais curta;
- deslocar o peso semântico do nome para a família, e não para o prefixo global;
- distinguir com clareza nomes atuais aceitos de nomes futuros desejáveis.

Exemplos apenas ilustrativos de direção futura, não de decisão já aceita:

- `tempo.agora_unix(...)`
- `tempo.formatar_unix(...)`
- `tempo.agora(...)`
- `tempo.formatar(...)`

Essas formas:

- não são sintaxe implementada;
- não são aliases aceitos agora;
- não congelam a decisão lexical final;
- servem apenas para mostrar o tipo de simplificação que o bloco pretende tornar possível no futuro.

## 6. Limites explícitos desta formalização

Esta fase não afirma:

- `familia.intrinseca` operacional;
- `trazer tempo;`;
- `trazer tempo.algo;`;
- biblioteca adulta de datas, calendário, timezone, locale ou duração;
- reclassificação de `tempo` como subsistema soberano já implementado.

## 7. Continuidade preparada

Com esta formalização, o Bloco 18 passa a ter um caso exemplar pequeno e estável para conduzir próximas rodadas sobre:

- documentação pública por família;
- leitura correta entre legado global e superfície temática futura;
- critérios de adoção lexical sem reorganização prematura da engine.
