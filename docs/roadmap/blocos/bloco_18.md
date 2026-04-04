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

## Estado factual atual
Bloco oficialmente aberto e ativo em camada documental/arquitetural. A Fase 180 abriu o bloco com inventário canônico de intrínsecas. A Fase 181 canonizou as famílias públicas iniciais (`texto`, `arquivo`, `caminho`, `processo`, `tempo`, `ambiente`, `acaso`) e deixou `colecao` e `formato` como domínios provisórios. A Doc-35 reduziu o custo de contexto do sistema histórico sem alterar esse estado.

## Limites explícitos
- não há `familia.intrinseca`;
- não há `trazer familia;` nem `trazer familia.intrinseca;`;
- não houve reorganização funcional de `src/`, `semantic.rs`, `interpreter.rs` ou `tests/`;
- não houve mudança funcional na linguagem.

## Relação com o histórico
- Execução factual preservada em `docs/history/phases/151a200.md` (Fases 180 e 181).
- A refatoração histórica que reduz duplicação contextual está em `docs/history/documentation/001a050.md` (Doc-35).
