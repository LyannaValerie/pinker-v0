# Apps Pinker

- **Classe:** Engine
- **Papel:** aplicacoes internas
- **Status:** ativo

`apps/` abriga programas escritos em Pinker que ajudam o desenvolvimento da propria Pinker.

Diferenca para `examples/`:

| Diretorio | Papel |
|---|---|
| `examples/` | demonstrar recursos pequenos e versionados por fase |
| `apps/` | manter programas praticos, com finalidade operacional real |

Contrato de entrada:

- todo app tem subdiretorio proprio;
- todo app tem `README.md`;
- o fonte principal se chama `principal.pink`;
- o comando de execucao precisa estar documentado;
- app usado no fluxo ativo deve ter teste automatizado.

## Apps Ativos

| App | Papel | Fluxo |
|---|---|---|
| `guardiao_pinker/` | auditor documental/operacional minimo do repositorio | rodar antes de proxima fase do Bloco 20, Eixo A |

