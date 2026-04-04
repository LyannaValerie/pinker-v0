# Sistema histórico da Pinker v0

- **Classe:** Engine
- **Papel:** histórico
- **Status:** ativo

`docs/history.md` é o ponteiro canônico curto do histórico.

A crônica factual continua canônica, mas agora vive no sistema shardado sob `docs/history/`.
O fluxo correto é:

1. ler este arquivo para entender precedência e arquitetura;
2. abrir `docs/history/indice.md` para localizar a categoria correta;
3. abrir o `indice.md` da categoria certa;
4. só então abrir o shard necessário.

## Precedência factual

- Este arquivo formaliza a existência do sistema histórico.
- `docs/history/indice.md` é o hub de navegação histórica.
- Os shards em `docs/history/*/*.md` carregam o conteúdo factual das entradas.
- Índice não é crônica: os índices roteiam; os shards preservam o conteúdo histórico.
