Refs #550
Refs #551
Refs #547
Refs #548
Refs #544
Refs #538

Extrai a superfície organizacional da Forja do repositório Pinker, preservando
produto Pinker, backend nativo, automação independente e a ponte operacional
mínima. A autoridade host-side existente permanece no Git local sem remote.

Inclui reconstrução histórica por `materialize-region` para as regiões correntes
removidas, sem alteração de medidas FROZEN, além do rebaseline mínimo da #548.

Gates locais: `PINKER_EXIGE_NATIVO=1 make ci`, Trama/nav/doc e projeções FROZEN
13/13 MATCH. AUTO_MERGE = FALSE.
