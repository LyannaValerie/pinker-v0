---
pinker-doc: 1
id: development.runtime-public-memory-containment
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - runtime.public-memory-containment
related:
  - development.deterministic-infrastructure-window
  - language
---

# Contenção da memória pública

- **Classe:** Engine
- **Papel:** contrato operacional
- **Status:** ativo

<!-- @pinker-doc:start
id: development.runtime-public-memory-containment.contract
tags: [runtime, memoria, demand-paging, paridade, testes]
aliases:
  - contencao de memoria publica
  - contrato de memoria publica
summary: Contrato do hotfix extraordinário que tornou a memória pública proporcional, lazy e contabilmente equivalente no interpretador e no runtime nativo.
-->

## Contexto do incidente e mecanismo confirmado

O hotfix foi autorizado diretamente pela mantenedora como correção extraordinária
de estabilidade e consumo de memória depois do merge humano da PR #420
(`a22735385fde0a55b2cd1b3a010a9a6063600bda`). Ele interrompe temporariamente a
janela auxiliar, não conta como uma de suas seis entregas, não abre Fase, não
inicia D2 e não autoriza GC, ownership ou reciclagem de endereços.

O mecanismo de crescimento foi confirmado no runtime nativo anterior: cada
processo reservava uma arena pública monolítica de 8 GiB e `alocar` executava
`write_bytes(0, tamanho)`, materializando ansiosamente todo o intervalo lógico.
A correção remove as duas causas. Cada alocação recebe um `mmap` anônimo
proporcional ao tamanho arredondado, sem toque inicial; demand paging realiza
fisicamente apenas as páginas acessadas e o kernel garante zero inicial.

## Baseline segura e resultados medidos

A medição usou um filho dedicado com `RLIMIT_CORE=0`, limite explícito de espaço
de endereçamento, limite de CPU e timeout de 5 segundos. Nenhuma alocação
ilimitada ou próxima de 7 GiB foi executada.

| Caso | Antes do hotfix | Implementação anterior | Implementação corrigida |
|---|---:|---:|---:|
| `alocar(64 MiB)`, sem acesso | VSZ ~3,2 MiB / RSS ~2 MiB | VSZ ~8,0 GiB / RSS ~66 MiB / 16.384 páginas residentes | VSZ ~67 MiB / RSS ~2,1 MiB / 0 páginas residentes |
| tocar início, meio e fim de 64 MiB | — | páginas já estavam todas residentes | 3 páginas residentes |
| `liberar` depois dos três toques | — | RSS volta próximo da base; arena de 8 GiB permanece | 0 páginas residentes; região proporcional permanece `PROT_NONE` |
| dois filhos de 16 MiB | — | cada processo começava com 8 GiB de VSZ | cada processo reserva apenas suas regiões |

Os três toques realizaram três páginas no host da baseline. Em kernels x86-64
com Transparent Huge Pages em modo `always`, cada toque pode realizar uma PMD
huge page de 2 MiB; o teste aceita essa granularidade do kernel, mas continua
recusando a materialização integral das 16.384 páginas.

O interpretador já era esparso: o exemplo versionado de 32 MiB ficou perto de
9,8 MiB de RSS, enquanto o nativo anterior ficou perto de 35 MiB. Isso confirma
uma divergência anterior de realização física, não atribui o workload histórico.

## Contrato de memória pública

`PUBLIC_PAGE_BYTES` permanece em 4096 bytes. O tamanho cobrado é sempre o tamanho
reservado arredondado por página (`1 → 4096`, `4097 → 8192`). Uma região
individual não pode exceder 256 MiB reservados.

| Recurso | Unidade | Limite | Recuperado por `liberar` |
|---|---|---:|---|
| identidades vitalícias | entrada histórica | 1.000.000 | não |
| espaço virtual vitalício | bytes de página mapeados na soma vitalícia | 8 GiB | não |
| regiões simultaneamente vivas | bytes de página reservados vivos | 256 MiB | sim, em bytes |
| metadata | 64 bytes por identidade publicada | 64 MiB | não |

Essas são quatro cotas independentes: identidades, virtual vitalício, reservado
vivo e metadata. Elas não representam a capacidade operacional do host. Em
particular, a cota de um milhão de identidades é um teto lógico vitalício, não
uma garantia de que o runtime nativo consiga manter um milhão de mapeamentos.

## Atomicidade, publicação e liberação

O veredicto puro compartilhado antecede qualquer efeito. Depois de validar
tamanho, largura, arredondamento e as quatro cotas, o runtime reserva metadata,
faz o mapeamento anônimo proporcional com `MAP_NORESERVE` e só então publica
identidade e contadores. Falha de metadata ou de mapeamento não publica estado
parcial. Falha do host ao criar o mapeamento preserva o diagnóstico controlado
`E-RUNTIME-MEM-PUBLIC-MAP`.

`liberar` executa `MADV_DONTNEED` e depois `PROT_NONE`. A região não recebe
`munmap` durante a vida do processo: endereço, identidade, virtual vitalício e
metadata não são reciclados; somente os bytes reservados vivos voltam a ficar
disponíveis. O kernel devolve os mapeamentos no encerramento do processo.

## Paridade e realização física

Interpretador e runtime nativo compartilham a mesma autoridade de orçamento e,
por isso, o mesmo arredondamento, ordem de consumo, recuperação, classe
diagnóstica e exit 1. A realização física permanece própria: o interpretador usa
armazenamento esparso; o nativo usa páginas anônimas sob demanda. Ambos observam
zero inicial antes da primeira escrita.

O runtime nativo instala `RLIMIT_CORE.soft=0` antes de executar código Pinker.
Essa defesa pertence à inicialização do runtime e não declara uma política geral
de contenção dos subprocessos da suíte ou do host.

## Limites arquiteturais e remaining_gaps

- **Concorrência de regiões mínimas:** a cobrança mínima é uma página de 4096
  bytes. Portanto, `256 MiB / 4096 bytes = 65.536 regiões vivas` de tamanho
  mínimo. Identidades vitalícias e regiões simultaneamente vivas são limites
  distintos.
- **Churn virtual vitalício:** `liberar` recupera bytes vivos, mas não recupera
  virtual vitalício. Um processo longo com alta rotatividade pode esgotar os
  8 GiB acumulados mesmo mantendo RSS estável.
- **Limites do host:** o runtime nativo pode receber falha de `mmap` antes dos
  tetos lógicos, inclusive por `vm.max_map_count`. A capacidade operacional do
  host não é inferida da cota de identidades; o erro permanece
  `E-RUNTIME-MEM-PUBLIC-MAP`.
- **Endereços mortos:** regiões liberadas continuam consumindo VSZ até o exit,
  intencionalmente, para impedir reutilização de endereço.
- **Portabilidade:** `MAP_NORESERVE`, `mincore`, `madvise`, `mprotect` e `/proc`
  tornam as provas de residência específicas de Linux.

## Atribuição histórica incompleta

O mecanismo de crescimento está identificado. O workload exato que consumiu
aproximadamente 7,18 GiB em 31/07 continua sem atribuição positiva. Registros
históricos de coredump podem ser compatíveis com testes conhecidos, mas não são
individualmente atribuídos sem correlação suficiente. O sufixo de hash produzido
pelo Cargo não identifica uma revisão Git.

Depois do merge humano do hotfix, a janela retoma sua segunda entrega ordinária:
snapshots e projeções da Issue #384. A PR #421 não declara a interrupção inteira
concluída antes desse merge.

<!-- @pinker-doc:end development.runtime-public-memory-containment.contract -->
