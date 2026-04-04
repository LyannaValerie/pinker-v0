# Domínios internos por intrínseca

- **Classe:** Ponte
- **Papel:** referência
- **Status:** ativo

Este documento registra a classificação canônica de domínios internos por intrínseca no **Bloco 18 — core nobre e bibliotecas temáticas**.

## 1. Leitura correta desta classificação

Domínio interno, nesta fase, significa:

- uma organização documental do inventário de intrínsecas;
- uma leitura funcional/coesa do que já existe hoje;
- uma base para futura evolução por família sem antecipar mecanismo operacional.

Domínio interno não significa, por si só:

- namespace funcional implementado;
- `familia.intrinseca`;
- `trazer familia;`;
- `trazer familia.algo;`;
- superfície pública já reorganizada no engine.

## 2. Domínios internos reconhecidos

Os domínios internos reconhecidos no estado atual do bloco são:

- `core`
- `texto`
- `arquivo`
- `caminho`
- `processo`
- `tempo`
- `ambiente`
- `acaso`
- `colecao` — provisório
- `formato` — provisório

## 3. Relação entre domínio, família e superfície

Leitura canônica:

- **domínio interno** organiza a leitura factual do inventário;
- **família pública** organiza a leitura arquitetural da superfície;
- **superfície futura por família** continua condicionada a mecanismo real.
- **resolução qualificada futura** é camada posterior e também continua dependente de mecanismo real.

Na maior parte dos casos aceitos hoje, domínio interno e família pública apontam para o mesmo recorte semântico.

Mesmo assim, as camadas não são idênticas:

- um domínio pode ser provisório sem virar família pública;
- uma família pública ainda pode continuar exposta por nomes globais legados;
- a existência de um domínio não autoriza docs a fingirem acesso qualificado já implementado;
- a existência de uma família pública não autoriza, por si só, resolução qualificada futura como se já estivesse pronta.

## 4. Domínio exemplar `tempo`

No caso exemplar `tempo`, a relação entre as camadas fica assim:

- domínio interno: `tempo`
- família pública aceita: `tempo`
- superfície atual: `tempo_unix()` e `formatar_tempo_unix(ts)`
- superfície futura plausível: forma temática sob `tempo`, ainda apenas documental/ilustrativa

## 5. Domínios provisórios

`colecao` e `formato` continuam provisórios.

Motivo canônico:

- o agrupamento técnico existe e já é útil para organizar o inventário;
- o nome público ainda não está maduro o bastante para canonização final;
- a futura superfície por família não deve se antecipar a essa revisão lexical.

## 6. Limites explícitos

Esta fase não abre:

- resolução qualificada;
- importação por família;
- aliases novos de migração;
- reorganização funcional de parser, semântica ou runtime.

Ela apenas fixa a classificação documental por domínio interno e sua relação com as demais camadas do Bloco 18.
