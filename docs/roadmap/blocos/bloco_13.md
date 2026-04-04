# Bloco 13 — coleções e estruturas de dados básicas

## Status
Encerrado.

## Tese
Abrir o mínimo auditável de coleções dinâmicas e iteração confortável para tornar a Pinker utilizável com dados variáveis.

## Dependências
Bloco 12 concluído.

## Escada interna
- array fixo operacional mínimo por valor;
- escrita mínima por índice em array fixo;
- `lista<bombom>` mínima;
- escrita mínima por índice em `lista<bombom>`;
- remoção mínima do fim em `lista<bombom>`;
- `mapa<verso,bombom>` mínimo;
- iteração confortável mínima sobre `lista<bombom>`;
- iteração confortável mínima sobre `mapa<verso,bombom>`;
- aleatoriedade básica com semente explícita.

## Estado factual atual
Bloco encerrado por suficiência conservadora na Fase 156, após as Fases 147–156, com lista, mapa, iteração mínima e aleatoriedade básica reproduzível.

## Limites explícitos
- sem generics;
- sem coleções amplas ou heterogêneas;
- sem random rico ou criptográfico.

## Relação com o histórico
- Execução factual preservada em `docs/history/phases/101a150.md` e `docs/history/phases/151a200.md`.
- Fechamento do bloco aparece no próprio encerramento factual da Fase 156 em `docs/history/phases/151a200.md`.
