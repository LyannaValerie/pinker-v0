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

Ela também não declara resolução qualificada futura como mecanismo já disponível; essa camada precisa de formulação própria e continua preparatória no Bloco 18.

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

## 7. Família exemplar do bloco

A família exemplar adotada para conduzir o Bloco 18 é **`tempo`**.

Razões canônicas:

- domínio pequeno e tecnicamente nítido;
- baixo risco lexical;
- dupla pública mínima já existente e coesa (`tempo_unix`, `formatar_tempo_unix`);
- boa relação entre utilidade prática e baixo custo conceitual;
- permite discutir família pública, legado, compatibilidade e futuro lexical sem arrastar um subsistema grande junto.

### 7.1 O que pertence hoje à família exemplar

Hoje pertencem à família `tempo` exatamente:

- `tempo_unix() -> bombom`
- `formatar_tempo_unix(ts) -> verso`

A Fase 186 abriu `trazer tempo;` como primeiro recorte funcional mínimo de importação por família. A Fase 187 ampliou esse mesmo mecanismo para `trazer ambiente;`, a Fase 188 o estendeu para `trazer acaso;` e a Fase 189 o estendeu para `trazer texto;`, sempre no menor recorte auditável e sem alterar a disponibilidade global das intrínsecas. Importação seletiva e resolução qualificada operacional permanecem fora do recorte.

### 7.2 Superfície mínima atual

A superfície pública mínima atual da família exemplar é a própria dupla legada já reconhecida pelo engine:

- `tempo_unix`
- `formatar_tempo_unix`

Leitura correta:

- hoje a família nasce sobre nomes globais legados preservados;
- a forma temática `tempo` já é reconhecida documentalmente;
- a migração para nomes mais curtos ou qualificados continua apenas como direção futura.

### 7.3 Compatibilidade e direção futura

Compatibilidade lexical explícita:

- `tempo_unix` permanece nome aceito e preservado;
- `formatar_tempo_unix` permanece nome aceito e preservado.

Direção lexical futura plausível, sem congelamento nesta fase:

- reduzir a redundância dos prefixos globais quando existir superfície por família;
- deslocar o sentido principal para `tempo` como domínio, não para o prefixo inteiro do identificador;
- estudar nomes futuros mais curtos sob família, sem transformar isso em obrigação imediata.

Exemplos apenas ilustrativos de direção futura:

- `tempo.agora_unix(...)`
- `tempo.formatar_unix(...)`
- `tempo.agora(...)`
- `tempo.formatar(...)`

Referência curta da formalização exemplar: `docs/familias/tempo.md`.

## 8. Preparação para 18.3

Esta decisão prepara 18.3 e seguintes do bloco ao deixar explícito:

- quais famílias já podem ser tratadas como oficiais no plano documental;
- quais agrupamentos ainda não devem ser congelados como nome público;
- qual família exemplar deve liderar a trilha;
- que a próxima etapa precisa discutir superfície e adoção por família sem fingir que o mecanismo já existe;
- que a ponte entre legado atual e direção lexical futura já está formalizada em um caso pequeno e auditável.

## 9. Superfície futura por família

A leitura canônica do Bloco 18 passa a ser:

- famílias públicas já existem como organização documental da superfície;
- a superfície implementada atual continua exposta por nomes globais legados;
- uma futura superfície por família depende de mecanismo real, não só de desejo lexical.

No caso exemplar `tempo`, isso significa:

- presente implementado: `tempo_unix` e `formatar_tempo_unix`;
- horizonte plausível: forma temática sob `tempo` com nomes internos potencialmente mais curtos;
- limite atual: ausência de namespace funcional, importação por família e resolução qualificada.

Critérios canônicos dessa futura transição:

1. preservar compatibilidade explícita;
2. só simplificar quando houver ganho nominal real;
3. deslocar o peso semântico do prefixo global para a família;
4. não canonizar cedo demais variantes ainda ilustrativas;
5. depender de mecanismo operacional real antes de vender a nova superfície como pronta.

Referências canônicas desta política:

- `docs/familias/dominios.md`
- `docs/familias/resolucao.md`
- `docs/familias/superficie.md`
- `docs/familias/tempo.md`

## 10. Relação com domínios internos

No estado atual do bloco:

- domínios internos organizam o inventário factual das intrínsecas;
- famílias públicas organizam a leitura arquitetural da superfície;
- a futura superfície por família continua sendo camada posterior e ainda não operacional.

Domínios internos reconhecidos com família pública aceita correspondente:

- `texto`
- `arquivo`
- `caminho`
- `processo`
- `tempo`
- `ambiente`
- `acaso`

No estado funcional atual de 18.6, apenas `tempo`, `ambiente`, `acaso` e `texto` estão importáveis via `trazer familia;`. As demais famílias públicas aceitas continuam apenas como organização documental/arquitetural.

Domínios internos que continuam provisórios nesta fase:

- `colecao`
- `formato`

O `core` permanece domínio interno do núcleo nobre, não família pública de biblioteca.

## 11. Resolução qualificada futura por família

No estado atual do bloco:

- famílias públicas aceitas já existem como decisão arquitetural;
- a superfície real continua global e legada;
- a futura resolução qualificada por família permanece apenas como direção preparada.

Leitura correta:

- `familia.intrinseca` ainda não funciona;
- exemplos qualificados em docs só são aceitáveis quando marcados explicitamente como ilustrativos;
- a futura abertura dessa camada precisa preservar compatibilidade com a superfície global atual;
- essa futura abertura não autoriza, por si só, importação por família.

Referência canônica desta preparação: `docs/familias/resolucao.md`.
