# Bloco 18 — core nobre e bibliotecas temáticas

## Status
Ativo.

## Tese
Separar com nitidez o núcleo da linguagem e suas famílias públicas de biblioteca, amadurecendo a nomeação temática sem fingir mecanismo funcional já implementado.

## Dependências
Bloco 17 encerrado; continuidade factual do motor técnico preservada.

## Escada interna
- inventário canônico das intrínsecas existentes;
- definição das famílias públicas iniciais;
- detalhamento de famílias exemplares;
- preparação da superfície futura por família sem reorganização funcional prematura.
- preparação documental da futura resolução qualificada por família antes de qualquer abertura operacional.

## Estado factual atual
Bloco oficialmente aberto e ativo em camada documental/arquitetural. A Fase 180 abriu o bloco com inventário canônico de intrínsecas. A Fase 181 canonizou as famílias públicas iniciais (`texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`) e deixou `colecao` e `formato` como domínios provisórios. A Fase 182 formalizou `tempo` como família exemplar do bloco sobre sua superfície mínima atual (`tempo_unix`, `formatar_tempo_unix`), preservando compatibilidade lexical e distinguindo estado legado de direção futura. A Fase 183 fixou a política documental da superfície futura por família a partir desse caso exemplar, distinguindo formas ilustrativas, critérios de transição e limites explícitos do estado implementado. A Fase 184 formalizou a leitura por domínio interno de intrínseca, distinguindo essa camada factual das famílias públicas e da futura superfície por família. A Fase 185 prepara documentalmente a futura resolução qualificada por família, distinguindo essa camada da superfície atual e fixando critérios mínimos para futura abertura operacional. A Doc-35 reduziu o custo de contexto do sistema histórico sem alterar essa trilha.

## Limites explícitos
- não há `familia.intrinseca`;
- não há `trazer familia;` nem `trazer familia.intrinseca;`;
- não há resolução qualificada por família já operacional;
- não houve reorganização funcional de `src/`, `semantic.rs`, `interpreter.rs` ou `tests/`;
- não houve mudança funcional na linguagem.

## Família exemplar atual

- Família exemplar do bloco: `tempo`.
- Superfície mínima atual: `tempo_unix()` e `formatar_tempo_unix(...)`.
- Leitura canônica: a família já existe como decisão documental/arquitetural, mas continua apoiada em nomes globais legados preservados.
- Ponte canônica para o futuro: a eventual simplificação temática depende de mecanismo real de superfície por família e de critérios explícitos de transição.
- Ponte canônica para a resolução qualificada futura: a eventual forma `familia.intrinseca` continua apenas preparatória e condicionada a mecanismo real mais contrato explícito de compatibilidade.
- Domínio interno correspondente: `tempo`.
- Revisão lexical futura permanece aberta, sem compromisso operacional nesta fase.

## Domínios internos atuais

- Domínios internos reconhecidos: `core`, `texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`, `colecao` (provisório) e `formato` (provisório).
- Leitura canônica: domínio interno organiza o inventário factual; família pública organiza a camada arquitetural; superfície futura por família continua condicionada a mecanismo real; resolução qualificada futura também permanece apenas como camada posterior preparada.

## Relação com o histórico
- Execução factual preservada em `docs/history/phases/151a200.md` (Fases 180, 181, 182, 183, 184 e 185).
- A refatoração histórica que reduz duplicação contextual está em `docs/history/documentation/001a050.md` (Doc-35).
