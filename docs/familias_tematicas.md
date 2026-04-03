# Famílias temáticas oficiais da Pinker

- **Classe:** Ponte
- **Papel:** híbrido
- **Status:** ativo

Este documento registra a decisão canônica inicial do **Bloco 18 — core nobre e bibliotecas temáticas** no estado documental atual do workspace.

## 1. Escopo da decisão

Esta rodada define apenas a camada **nominal e arquitetural** das famílias públicas.

Ela **não**:

- abre resolução qualificada `familia.intrinseca`;
- abre `trazer familia;` ou `trazer familia.intrinseca;`;
- reorganiza `src/`, `tests/`, `semantic.rs` ou `interpreter.rs`;
- declara famílias como já operacionais em código;
- introduz sintaxe nova, aliases novos ou modo estrito.

## 2. Base factual usada nesta decisão

A decisão foi ancorada na continuidade aberta pela **Fase 180**, que registrou o inventário canônico de intrínsecas do **Bloco 18**, e na superfície pública real rastreável no workspace atual, conferida em:

- o inventário canônico aberto na Fase 180 como base factual anterior da trilha;
- `docs/vocabulario.md` como registro lexical canônico da superfície pública;
- `src/semantic.rs` e `src/interpreter.rs` como evidência operacional da lista pública realmente reconhecida hoje.

Esta fase não inventa uma implementação ausente; ela continua a organização nominal/arquitetural a partir da base factual já consolidada na abertura do bloco.

## 3. Critérios de aceitação lexical

Uma família pública inicial só deve ser aceita quando reúne, ao mesmo tempo:

1. força técnica real no conjunto já existente de intrínsecas;
2. clareza de domínio para docs e uso futuro;
3. sustentabilidade de crescimento sem colapsar o nome;
4. legibilidade alta em código, docs e diagnósticos;
5. coerência com a identidade lexical da Pinker;
6. capacidade de sustentar futura documentação pública por família;
7. baixo risco de confusão com construtos ou categorias já existentes.

Quando o agrupamento funcional existe, mas o nome ainda parece imaturo, amplo demais ou lexicalmente instável, o domínio permanece **provisório**.

## 4. Famílias públicas iniciais aceitas

As famílias públicas iniciais aceitas nesta fase são:

- `texto`
- `arquivo`
- `caminho`
- `processo`
- `tempo`
- `ambiente`
- `acaso`

Estas famílias passam a existir **como decisão canônica de nome e agrupamento**, não como superfície já qualificada em parser/runtime.

## 5. Domínios provisórios explícitos

Os domínios abaixo permanecem provisórios:

- `colecao`
- `formato`

Motivo canônico:

- `colecao` descreve um agrupamento real já visível (`lista_*`, `mapa_*`), mas ainda carrega risco de congelar cedo demais um nome público para uma superfície ainda muito recortada e heterogênea.
- `formato` descreve um agrupamento funcional reconhecível (`formatar_verso`, CSV mínimo, JSON plano mínimo), mas ainda mistura serialização, formatação textual e dados estruturados sob um rótulo lexical que pede maturação antes de virar família pública oficial.

## 6. Famílias em revisão lexical

Nesta fase, entram em revisão lexical explícita:

- `colecao`
- `formato`

O estado de revisão lexical significa:

- o domínio técnico existe;
- o nome ainda não foi canonizado como família pública;
- a futura implementação por família não deve se antecipar a essa decisão.

## 7. Família exemplar recomendada

A família exemplar recomendada para conduzir o Bloco 18 é **`tempo`**.

Razões:

- domínio pequeno e tecnicamente nítido;
- baixo risco lexical;
- já possui dupla pública coesa (`tempo_unix`, `formatar_tempo_unix`);
- é a família mais adequada para inaugurar discussão futura de documentação e eventual resolução qualificada sem arrastar um subsistema grande junto.

## 8. Preparação para 18.3

Esta decisão prepara 18.3 e seguintes do bloco ao deixar explícito:

- quais famílias já podem ser tratadas como oficiais no plano documental;
- quais agrupamentos ainda não devem ser congelados como nome público;
- qual família exemplar deve liderar a trilha;
- que a próxima etapa precisa discutir superfície e adoção por família sem fingir que o mecanismo já existe.
