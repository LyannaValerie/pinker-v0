# Superfície futura por família

- **Classe:** Ponte
- **Papel:** referência
- **Status:** ativo

Este documento registra a leitura canônica da superfície futura por família no **Bloco 18 — core nobre e bibliotecas temáticas**.

Ele existe para evitar uma confusão recorrente:

- a Pinker já reconhece famílias públicas no plano documental;
- a Pinker ainda expõe sua superfície pública real principalmente por nomes globais legados;
- a existência documental de uma família não implica namespace funcional já implementado.
- a futura resolução qualificada por família precisa de política própria antes de qualquer abertura operacional.

## 1. Estado implementado atual

Hoje, a Pinker:

- já pode ser lida documentalmente por domínios internos de intrínseca;
- aceita famílias públicas como decisão nominal/arquitetural;
- continua expondo as intrínsecas existentes por nomes globais legados;
- ainda não possui `familia.intrinseca`;
- possui `trazer familia;` apenas para as famílias `tempo`, `ambiente`, `acaso` e `texto` (Fases 186–189, recorte mínimo);
- ainda não possui `trazer familia.algo;` (importação seletiva).

Leitura correta:

- a família existe como organização canônica da superfície;
- `trazer tempo;`, `trazer ambiente;`, `trazer acaso;` e `trazer texto;` são o mecanismo real atual de importação por família, com efeito operacional mínimo (as intrínsecas já estão disponíveis globalmente; o import é válido mas não obrigatório);
- outras famílias ainda não são importáveis;
- a documentação não deve reescrever o presente como se importação geral por família já existisse.

## 2. Caso exemplar `tempo`

No caso exemplar `tempo`, a superfície global legada atual é:

- `tempo_unix() -> bombom`
- `formatar_tempo_unix(ts) -> verso`

Esses nomes continuam:

- aceitos;
- canônicos no estado implementado;
- preservados por compatibilidade lexical e histórica.

## 3. Superfície futura plausível

Quando a Pinker tiver mecanismo próprio para superfície por família, a direção arquitetural plausível é deslocar o peso semântico do nome para a família.

Isso sugere, em termos abstratos:

- menos dependência do prefixo global inteiro;
- nomes internos de família potencialmente mais curtos;
- leitura pública centrada no domínio temático.

No caso exemplar `tempo`, isso pode futuramente significar algo como:

- uma forma qualificada sob `tempo`;
- nomes internos menores do que `tempo_unix` e `formatar_tempo_unix`;
- preservação explícita da distinção entre “agora/obter” e “formatar”.

Importante:

- esta fase não canoniza spelling final;
- esta fase não escolhe sintaxe final de resolução;
- esta fase não cria aliases novos;
- esta fase não define migração operacional.

Referência específica da camada seguinte: `docs/familias/resolucao.md`.

## 4. Formas ilustrativas x formas canônicas

Formas futuras mostradas em docs do Bloco 18 devem ser lidas como **ilustrativas** quando servirem apenas para mostrar direção arquitetural.

Elas:

- não são nomes aceitos pela linguagem hoje;
- não são compromisso lexical final;
- não autorizam docs de uso a tratá-las como prontas.

Forma canônica nesta fase significa:

- nome aceito agora no estado implementado; ou
- decisão documental explícita sobre critérios, limites e direção.

## 5. Critérios de transição

Uma futura transição entre nomes globais legados e superfície por família deve obedecer, no mínimo, estes critérios:

1. **Compatibilidade explícita** — não quebrar a leitura atual sem registrar o contrato de transição.
2. **Ganho nominal real** — a nova forma precisa reduzir redundância ou ruído com benefício claro.
3. **Peso semântico na família** — o domínio deve carregar mais sentido do que o prefixo global herdado.
4. **Clareza de par funcional** — nomes irmãos devem continuar distinguindo bem operações diferentes.
5. **Base operacional real** — simplificação lexical forte só faz sentido quando existir mecanismo real de superfície por família.
6. **Não congelar cedo demais ilustrações** — exemplos de direção não viram naming final por inércia.
7. **Honestidade documental** — docs de roadmap, léxico e famílias devem continuar separando presente implementado de horizonte futuro.

## 6. Limites explícitos desta política

Esta política não define:

- parser novo;
- semântica nova;
- runtime novo;
- importação por família;
- namespacing funcional;
- cronograma obrigatório de migração.

Ela apenas fixa como o Bloco 18 deve descrever a ponte entre:

- domínios internos por intrínseca;
- superfície global legada;
- superfície temática futura;
- futura resolução qualificada por família;
- critérios que governarão essa transição.
