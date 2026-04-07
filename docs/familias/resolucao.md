# Resolução qualificada futura por família

- **Classe:** Ponte
- **Papel:** referência
- **Status:** ativo

Este documento registra a leitura canônica da futura resolução qualificada por família no **Bloco 18 — core nobre e bibliotecas temáticas**.

## 1. O que esta resolução significa

No Bloco 18, falar em **resolução qualificada por família** significa preparar a forma pública em que uma intrínseca poderá, no futuro, ser acessada por um identificador de família.

Nesta fase, isso significa apenas:

- uma direção arquitetural preparada;
- uma camada documental distinta da superfície global legada;
- um alvo futuro condicionado a mecanismo real.

Não significa:

- `familia.intrinseca` já implementada;
- `trazer familia;` generalizado (apenas `tempo` e `ambiente` nas Fases 186–187);
- `trazer familia.algo;` (importação seletiva, ainda fora do recorte);
- parser, semântica ou runtime reorganizados amplamente.

## 2. Relação com a superfície atual

Hoje, a superfície pública real continua apoiada em **nomes globais legados**.

No caso exemplar `tempo`, isso significa:

- `tempo_unix()`
- `formatar_tempo_unix(ts)`

Esses nomes continuam:

- aceitos;
- canônicos no presente implementado;
- preservados por compatibilidade lexical e histórica.

A futura resolução qualificada não apaga automaticamente essa superfície. Ela só pode ser aberta com contrato explícito de compatibilidade.

## 3. Formas plausíveis e formas ilustrativas

Uma forma qualificada futura plausível é aquela que:

- desloca o peso semântico do nome para a família;
- reduz redundância herdada do prefixo global;
- preserva distinções funcionais importantes;
- depende de mecanismo operacional real.

No caso exemplar `tempo`, exemplos como:

- `tempo.agora_unix(...)`
- `tempo.formatar_unix(...)`
- `tempo.agora(...)`
- `tempo.formatar(...)`

devem ser lidos nesta fase apenas como **ilustração arquitetural**.

Esses exemplos:

- não são aceitos hoje;
- não congelam a decisão lexical final;
- não autorizam docs de uso a tratá-los como prontos.

## 4. Relação entre as camadas do bloco

Leitura canônica:

- **domínio interno** organiza o inventário factual;
- **família pública** organiza a leitura arquitetural da superfície;
- **superfície futura por família** descreve a direção pública geral;
- **resolução qualificada futura** descreve a futura forma de acesso por família, quando houver mecanismo real.

Essas quatro camadas se relacionam, mas não podem ser colapsadas em uma só.

## 5. Critérios mínimos de futura abertura operacional

Antes de qualquer fase operacional real, a Pinker precisa satisfazer, no mínimo:

1. **Mecanismo real de resolução** — a linguagem precisa de suporte objetivo para acessar intrínsecas por família.
2. **Contrato explícito de compatibilidade** — a relação entre forma legada e forma qualificada precisa estar documentada.
3. **Ganho nominal e arquitetural claro** — a nova forma precisa melhorar legibilidade ou organização de modo defensável.
4. **Distinção estável entre forma aceita e ilustração** — exemplos documentais não podem virar naming final por inércia.
5. **Separação de escopo** — resolução qualificada futura não implica, por si só, importação por família.
6. **Honestidade documental contínua** — docs de estado e uso não podem reescrever o presente como se a forma futura já existisse.

## 6. Limites explícitos desta preparação

Esta fase não:

- implementa `familia.intrinseca`;
- implementa `trazer familia;` ou `trazer familia.algo;`;
- escolhe spelling final obrigatório;
- abre cronograma de migração;
- reorganiza `src/`, `semantic.rs`, `interpreter.rs` ou `tests/`.

Ela apenas fixa a leitura preparatória da futura resolução qualificada por família e seus limites canônicos.
