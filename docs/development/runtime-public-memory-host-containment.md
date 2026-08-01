---
pinker-doc: 1
id: development.runtime-public-memory-host-containment
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - runtime.public-memory-containment
  - development.native-execution-hygiene
related:
  - development.deterministic-infrastructure-window
  - language
---

# Memória pública e contenção do host

- **Classe:** Engine
- **Papel:** contrato operacional
- **Status:** ativo

<!-- @pinker-doc:start
id: development.runtime-public-memory-host-containment.contract
tags: [runtime, memoria, processos, coredump, testes]
aliases:
  - contencao de memoria publica
  - higiene de execucao nativa
summary: Contrato do hotfix extraordinário que tornou a memória pública lazy e conteve as execuções nativas controladas.
-->

## Contexto e atribuição

O hotfix foi autorizado diretamente pela mantenedora como correção extraordinária
de estabilidade e consumo de memória depois do merge humano da PR #420
(`a22735385fde0a55b2cd1b3a010a9a6063600bda`). Ele interrompe temporariamente a
janela auxiliar, não conta como uma de suas seis entregas, não abre Fase, não
inicia D2 e não autoriza GC, ownership ou reciclagem de endereços.

A recaptura confirmou duas causas no código anterior: cada processo reservava
uma arena única de 8 GiB na inicialização e `alocar` escrevia zero em todo o
intervalo solicitado. O workload exato observado em 31/07 por volta de 09:04
não pôde ser atribuído com a evidência disponível. O sufixo de hash produzido
pelo Cargo não identifica uma revisão Git; falhas controladas agora registram
Git HEAD e SHA-256 do ELF e da `libpinker_rt.a` separadamente.

## Baseline segura

A medição usou um filho dedicado, `RLIMIT_CORE=0`, espaço de endereçamento
limitado, timeout de 5 segundos, saída limitada e o runtime realmente linkado.
Nenhuma alocação de 7–8 GiB foi executada.

| Caso | Antes | Implementação anterior | Implementação corrigida |
|---|---:|---:|---:|
| `alocar(64 MiB)`, sem acesso | VSZ ~3,2 MiB / RSS ~2 MiB | VSZ ~8,0 GiB / RSS ~66 MiB / 16.384 páginas residentes | VSZ ~67 MiB / RSS ~2,1 MiB / 0 páginas residentes |
| tocar início, meio e fim de 64 MiB | — | páginas já estavam todas residentes | 3 páginas residentes |
| `liberar` depois dos três toques | — | RSS volta próximo da base; arena de 8 GiB permanece | 0 páginas residentes; região proporcional permanece `PROT_NONE` |
| dois filhos de 16 MiB | — | cada processo começava com 8 GiB de VSZ | cada processo reserva apenas suas regiões |

O interpretador já era esparso: o exemplo versionado de 32 MiB ficou perto de
9,8 MiB de RSS, enquanto o nativo anterior ficou perto de 35 MiB. Isso confirmava
divergência de realização, não o workload histórico exato.

## Contrato de memória pública

| Recurso | Unidade | Limite | Recuperado por `liberar` |
|---|---|---:|---|
| identidade | entrada histórica | 1.000.000 | não |
| virtual vitalício | bytes de página já mapeados | 8 GiB | não |
| reservado vivo | bytes de página de regiões vivas | 256 MiB | sim |
| metadata | carga canônica por entrada | 64 MiB (64 bytes/entrada) | não |

Uma região individual não pode exceder 256 MiB reservados. A página pública é
4096 bytes e a contabilidade usa o tamanho arredondado (`1 → 4096`, `4097 →
8192`). Os valores preservam a cota vitalícia publicada, acomodam oito vezes o
maior caso versionado anterior (32 MiB) e limitam quatro processos no teto vivo
a aproximadamente 1 GiB lógico agregado no host medido.

O veredicto puro antecede qualquer efeito. Depois de validar tamanho,
arredondamento e todas as cotas, a implementação reserva metadata, faz `mmap`
anônimo proporcional com `MAP_NORESERVE` e só então publica identidade e
contadores. Falha de mapeamento não consome estado. Páginas novas são zeradas
pelo kernel e não são tocadas durante `alocar`.

`liberar` executa `MADV_DONTNEED` e depois `PROT_NONE`. A região não recebe
`munmap` durante a vida do processo: endereço, identidade, virtual vitalício e
metadata não são reciclados; somente reservado vivo volta a ficar disponível.
O kernel devolve os mapeamentos no encerramento do processo.

Interpretador e nativo compartilham a mesma autoridade de orçamento e, por
isso, o mesmo arredondamento, ordem de consumo, classe diagnóstica e exit 1. A
realização permanece própria: HashMap esparso no interpretador, páginas anônimas
sob demanda no nativo.

## Contenção de execução nativa

Toda execução de ELF nas suítes nativas mapeadas passa por
`tests/common/native_process.rs`. A autoridade cria grupo de processo, instala
`PR_SET_PDEATHSIG=SIGKILL` com recaptura do pai, aplica core zero e limites de
espaço/CPU antes do `exec`, usa timeout por classe, termina o grupo inteiro e
limita stdout e stderr a 1 MiB por canal. Falhas registram caso lógico, Git HEAD,
hashes do executável/runtime, PID, PGID, política aplicada, início, duração e
status ou sinal.

Cada comando e conjunto multi-etapa usa diretório marcado sob
`target/pinker-exec/`. O caminho normal remove e confirma a remoção; `Drop`
também cobre erro ou panic. `scripts/pinker-cleanup.sh` é dry-run por padrão e só
remove com `--apply` uma entrada marcada, antiga e sem owner vivo com o mesmo
PID **e** start time. A ferramenta não segue symlinks, não aceita outra raiz e é
idempotente. `make cleanup-native` apenas inspeciona.

O runtime aplica `RLIMIT_CORE.soft=0` antes do código Pinker e o harness aplica
core zero antes de `exec`, cobrindo falhas anteriores à inicialização. O
`ci_env.sh` também desabilita core para toda a esteira. Pinker v0 não produz core
dump por padrão nas superfícies controladas; nenhuma configuração global do host
é alterada.

## Coredumps e resíduos históricos

O host usava `systemd-coredump`, não core files tradicionais no workspace. A
consulta mínima encontrou 83 entradas associadas a executáveis Pinker: 70
`SIGSEGV` e 13 `SIGABRT`. Seis entradas entre 08:00 e 10:00 de 31/07 tinham nome
do caso deliberado de endereço fabricado e `SIGSEGV`, mas nenhuma metadata
disponível demonstrou vínculo com o evento exato de 09:04. Classificação:

- incidente de memória confirmado: 0;
- teste conhecido por sinal, com relação temporal suportada: 6;
- defeito independente confirmado: 0;
- evidência insuficiente: 77.

Nenhuma entrada de `systemd-coredump` foi removida: uma exclusão seletiva e
segura não era necessária para corrigir o produtor e poderia afetar evidência do
host. Diretórios antigos em `/tmp` sem marcador novo também permanecem
preservados. A ferramenta de recuperação só reconhece resíduos futuros que a
Pinker consegue provar como próprios.

## Limites residuais

- a atribuição histórica do workload e dos 77 dumps restantes continua aberta;
- mapeamentos mortos consomem VSZ até o exit, intencionalmente, para impedir
  reutilização de endereço;
- `MAP_NORESERVE`, `mincore`, `madvise`, `mprotect`, `prctl` e `/proc` tornam as
  provas de realização e processos específicas de Linux;
- os limites do harness protegem execuções controladas pelo repositório, não
  processos externos iniciados fora dessas superfícies.

Depois do merge humano do hotfix, a janela retoma sua segunda entrega ordinária:
snapshots e projeções da Issue #384.

<!-- @pinker-doc:end development.runtime-public-memory-host-containment.contract -->
