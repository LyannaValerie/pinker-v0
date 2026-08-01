---
pinker-doc: 1
id: development.native-execution-host-containment
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - testing.native-execution-host-containment
  - testing.native-sandbox-recovery
related:
  - development.runtime-public-memory-containment
  - development.deterministic-infrastructure-window
---

# Contenção do host para execução nativa

- **Classe:** Engine
- **Papel:** contrato operacional
- **Status:** ativo

<!-- @pinker-doc:start
id: development.native-execution-host-containment.contract
tags: [testes, processos, sandbox, cleanup, proveniencia, linux]
aliases:
  - contencao de execucao nativa
  - autoridade de processos nativos
  - cleanup de sandboxes
summary: Contrato da segunda unidade de integração do HF-9 para conter processos, saída, recursos, sandboxes e recuperação nas suítes nativas.
-->

Este documento é canônico para a infraestrutura de testes que monta objetos,
liga ELF, executa binários Pinker ou inicia ferramentas externas capazes de
permanecer ativas. Ele não redefine o contrato de memória pública. O HF-9 possui
duas unidades de integração: a PR #421 entregou memória pública; esta unidade
entrega contenção do host. Nenhuma delas conta entre as seis entregas da janela
auxiliar. Depois do merge humano da segunda unidade, a entrega ordinária volta à
Issue #384; D2 continua não iniciado.

## Autoridade comum e suítes mapeadas

`tests/common/native_process.rs` fornece `ControlledCommand` e
`NativeArtifactDir`. A autoridade aplica política por classe, timeout,
`RLIMIT_AS`, `RLIMIT_CPU`, core zero, PGID próprio, terminação TERM/KILL, espera
do filho direto, reaping do supervisor e captura limitada quando solicitada.

As suítes mapeadas são as de backend nativo, toolchain externa, carga de listas,
SIGPIPE nativo, atribuição por `sussurro`, ponteiros nativos, fases 245–248,
evidências HR3 e contabilidade de uniões. O guardião remove literais e
comentários antes de procurar escapes e permite comandos diretos somente nos
testes internos deliberados da autoridade.

`tests/public_memory_hotfix_tests.rs` permanece excluído da migração mecânica:
ele possui executor dedicado, limites de memória mais estreitos e regressões do
contrato público. Testes de CLI, catálogo e interpretador que só executam
processos locais curtos também permanecem fora. A exclusão não autoriza novas
ferramentas externas nas suítes mapeadas.

## Raiz autorizada e sandbox

A raiz do repositório é descoberta pelo cwd real e pela entrada `.git`; nenhuma
variável de ambiente escolhe raiz alternativa. `target` e
`target/pinker-exec` são preparados componente por componente:

1. `symlink_metadata` inspeciona cada componente antes de qualquer travessia;
2. symlink ou componente que não seja diretório real é rejeitado;
3. diretórios ausentes são criados somente sob o pai já validado;
4. a canonicalização ocorre depois da validação;
5. a raiz canônica precisa permanecer sob o repositório canônico;
6. device e inode da raiz são registrados e revalidados antes de remoção.

`ExecutionSandbox`, `NativeArtifactDir`, scavenger e cleanup reutilizam essa
autoridade. Erro estrutural da raiz bloqueia a execução. Erro isolado de uma
entrada do scavenger gera `PRESERVED` e permite examinar as demais. Nenhuma
operação segue symlink, remove por nome global ou entrega `TMPDIR` e
`PINKER_EXECUTION_DIR` antes de a raiz ser provada.

## Semântica de stdio

`output()` preserva o contrato de `std::process::Command`: sem stdin explícito,
instala `Stdio::null()` e o filho recebe EOF imediato; stdout e stderr são
capturados. Configuração explícita de qualquer canal é preservada. A captura
padrão é limitada a 1 MiB por canal; exceder o teto encerra a árvore e conserva
no máximo o prefixo permitido.

`status()` é um caminho independente. Stdin, stdout e stderr são herdados por
padrão, configurações explícitas são respeitadas e não existem buffers ocultos.
A operação ainda recebe PGID, supervisor, limites, timeout e reaping.

## Identidade Linux em /proc

O parser localiza o último delimitador válido `) ` do campo `comm`, interpreta o
sufixo a partir do campo de estado e lê `starttime` na posição 22 do formato
completo. Conteúdo truncado, campo não numérico, ausência de `/proc` e qualquer
ambiguidade resultam em identidade `Unknown`.

A classificação é:

- `Live`: PID e start time correspondem;
- `Reused`: PID existe com start time diferente;
- `Missing`: `/proc/PID` comprovadamente não existe;
- `Unknown`: não foi possível provar identidade.

Somente `Missing` e `Reused` podem tornar um diretório antigo elegível.
`Unknown` sempre produz `PRESERVED`. Os testes usam um owner Linux real cujo
`comm` é `pi nker) x` e calculam o valor esperado por uma autoridade independente
baseada no nome exato observado.

## Árvore de processos e morte do controlador

O processo controlado lidera PGID próprio e recebe `PR_SET_PDEATHSIG` para a
fronteira direta. Um supervisor separado mantém a ponta de leitura de um canal
de vida `O_CLOEXEC`. Fechamento normal envia um byte de conclusão e não sinaliza
o PGID. EOF, inclusive após `SIGKILL` do controlador, envia TERM ao grupo,
aguarda 200 ms e envia KILL.

Depois do `fork`, o supervisor fixa o canal de vida no descritor 3 e fecha
todos os descritores de 4 em diante com `close_range`. Falha dessa higienização
encerra o supervisor de forma fechada; assim ele não retém arquivos, pipes ou
ELFs graváveis abertos por outra thread do controlador.

O controlador monitora o supervisor. Se o supervisor morrer primeiro, a
execução falha como `watchdog_exit`, o grupo é encerrado e o filho direto é
reaped. A arquitetura não depende do convidado nem de netos herdarem
`PR_SET_PDEATHSIG`. Testes distintos cobrem timeout, excesso em stdout/stderr,
morte normal, `SIGKILL` do controlador, morte do supervisor, filho e neto,
zumbi em reaping, recuperação do sandbox e execução saudável posterior.

## Cleanup e recuperação

`scripts/pinker-cleanup.sh` é dry-run por padrão e exige `--apply`. Ele valida a
raiz canônica e sua identidade, examina apenas nomes `exec-PID-ID`, exige
marcador regular e unívoco, idade mínima e owner comprovadamente inativo. A
saída distingue `STALE`, `PRESERVED` e `ERROR`. O `rm -rf` ocorre apenas depois
de todas as provas e de uma revalidação imediatamente anterior.

A ferramenta é idempotente, não segue symlink e não remove conteúdo externo.
Marcador ausente, duplicado, incompleto ou symlink é preservado. Falha de parser
de `/proc` também é preservada.

## Proveniência e diagnóstico

Em falhas, a autoridade registra caso lógico, Git HEAD real, SHA-256 do
executável, SHA-256 do runtime quando aplicável, PID, PGID, PID do supervisor,
classe de política, espaço de endereçamento, CPU, timeout, teto de captura,
início, duração, status, sinal e motivo de terminação. O hash do nome produzido
pelo Cargo nunca é tratado como revisão Git.

## Política de core da esteira

As fronteiras são complementares:

- **runtime Pinker:** a PR #421 instala `RLIMIT_CORE.soft=0` em
  `pinker_rt_iniciar`;
- **binários Rust de teste:** `ci_env.sh` instala soft core zero antes do Cargo;
- **compilador Pinker:** herda soft core zero da esteira antes de executar;
- **ferramentas externas:** herdam a mesma política em toda a árvore;
- **ambiente CI:** usa `ulimit -S -c 0`, preservando o hard limit do operador.

A política não altera `core_pattern`, não exige privilégio, não remove coredumps
históricos e não depende do runtime Pinker. Entradas de metadata do
`systemd-coredump` sem payload podem continuar no journal: o contrato impede
payloads e arquivos crescentes, não apaga história.

## Limites honestos

A implementação Linux não usa cgroups privilegiados, não muda
`vm.max_map_count` e não promete superar limites de VMA do host. Um milhão de
identidades permanece teto lógico; padrões alternados RW/`PROT_NONE` podem
impedir fusão; falha de `mmap` continua observável como
`E-RUNTIME-MEM-PUBLIC-MAP`. Números e semântica da memória pública pertencem ao
documento relacionado, não a esta autoridade.

<!-- @pinker-doc:end development.native-execution-host-containment.contract -->
