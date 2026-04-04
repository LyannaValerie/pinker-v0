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

## 5. Ponte para a superfície futura

A ponte canônica entre presente e futuro é esta:

- hoje `tempo` existe como família documental, mas sua superfície pública continua global;
- amanhã uma superfície por família só fará sentido quando houver mecanismo real para sustentá-la;
- a transição futura deve mover peso semântico do prefixo global para a família, sem apagar o contrato legado já aceito.

No caso exemplar `tempo`, isso significa:

- partir de `tempo_unix` e `formatar_tempo_unix` como base implementada real;
- admitir futura simplificação temática sob `tempo`;
- não tratar exemplos ilustrativos como se já fossem nomes aceitos.

Política canônica da transição: `docs/familias/superficie.md`.
Política canônica da futura resolução qualificada: `docs/familias/resolucao.md`.

## 6. Direção lexical futura, sem compromisso operacional

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
- não substituem a superfície global legada atual;
- servem apenas para mostrar o tipo de simplificação que o bloco pretende tornar possível no futuro.

Se a Pinker vier a abrir resolução qualificada por família, `tempo` continua sendo o caso exemplar mínimo para essa abertura:

- por ter só duas intrínsecas públicas;
- por preservar contraste funcional simples entre “obter/agora” e “formatar”;
- por permitir testar a relação entre nome global legado e acesso qualificado futuro sem inflar o escopo do bloco.

## 7. Critérios de transição aplicados ao caso exemplar

A futura transição de `tempo` deve obedecer, no mínimo:

1. preservar compatibilidade explícita com a superfície atual;
2. só reduzir nomes quando houver ganho nominal real;
3. deslocar o peso semântico para a família `tempo`;
4. manter clara a distinção entre “obter/agora” e “formatar”;
5. não congelar cedo demais formas hoje apenas ilustrativas;
6. depender de mecanismo real de superfície por família, e não de reinterpretação documental isolada;
7. distinguir com nitidez formas apenas ilustrativas de qualquer forma qualificada futuramente aceita.

## 8. Limites explícitos

A Fase 186 abriu `trazer tempo;` no recorte mínimo. Os limites atuais são:

- `familia.intrinseca` ainda não operacional;
- `trazer tempo.algo;` (importação seletiva) ainda não suportado;
- biblioteca adulta de datas, calendário, timezone, locale ou duração fora do escopo;
- `trazer tempo;` não cria obrigação de import; os nomes globais legados continuam válidos sem o import;
- nenhuma outra família além de `tempo` pode ser importada nesta fase.

## 9. Estado atual e continuidade

A Fase 186 abriu `trazer tempo;` como primeiro recorte funcional real de 18.6. Com isso:

- `trazer tempo;` é aceito pelo checker e pelo runtime;
- as intrínsecas `tempo_unix` e `formatar_tempo_unix` continuam disponíveis globalmente;
- a presença de `trazer tempo;` não é obrigatória nem altera o comportamento;
- próximas rodadas podem ampliar 18.6 para outras famílias ou avançar para 18.7.
