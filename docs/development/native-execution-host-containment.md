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
`NativeArtifactDir`; as autoridades locais de launcher, marcador e sandbox
ficam, respectivamente, em `native_process_launcher.rs`,
`native_process_marker.rs` e `native_process_sandbox.rs`. A autoridade aplica
política por classe, timeout, `RLIMIT_AS`, `RLIMIT_CPU`, core zero, PGID próprio,
TERM/KILL, reaping e captura limitada quando solicitada.

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
6. device e inode da raiz são registrados e revalidados antes de remoção;
7. nome, device e inode de cada `exec-*` são registrados na criação e cruzados
   com o marcador antes de qualquer elegibilidade.

`ExecutionSandbox`, `NativeArtifactDir`, scavenger e cleanup reutilizam essa
autoridade. Erro estrutural da raiz bloqueia a execução. Erro isolado de uma
entrada do scavenger gera `PRESERVED` e permite examinar as demais. Nenhuma
operação segue symlink, remove por nome global ou entrega `TMPDIR` e
`PINKER_EXECUTION_DIR` antes de a raiz ser provada. Identidade incerta preserva
o objeto e a evidência.

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

## Launcher permanente, handshake e lifecycle

A topologia é `controlador -> {watchdog, launcher -> convidado ->
descendentes}`. O launcher estabelece PGID próprio, ativa
`PR_SET_CHILD_SUBREAPER` e permanece como líder até não existir processo
executável nem filho recolhível da execução. Ele nunca faz `exec` sobre o
convidado. TERM é enviado aos descendentes por PIDFD, seguem 200 ms de espera e
KILL por PIDFD; o launcher é excluído desses sinais, ancora o número do PGID
durante toda a sequência, recolhe os processos sob sua autoridade e só então
termina.

A inicialização usa canais `O_CLOEXEC` distintos para prontidão, gate, controle
e vida. A ordem é obrigatória: raiz e sandbox provados, marcador `preparing`,
launcher pronto com PGID estabelecido e convidado ausente, watchdog pronto com
canal de vida e identidade do launcher, marcador `watchdog-ready` e, somente
então, mensagem explícita `G` no gate. EOF antes de `G` cancela; não autoriza o
convidado. Falha em qualquer etapa pré-gate termina e aguarda launcher e
watchdog antes de liberar a limpeza.

O watchdog registra PID, start time e PGID do launcher e conserva um canal
direto cujo único leitor é esse launcher; ele nunca sinaliza um PGID apenas por
número. O launcher e o watchdog observam canais independentes de vida do
controlador, e o controlador observa `waitpid(WNOHANG)` do watchdog. Morte do
watchdog após o gate produz `watchdog_exit`; morte do controlador produz EOF e
ordena encerramento da árvore. `PR_SET_PDEATHSIG` do convidado é defesa adicional,
não fundamento da autoridade sobre netos.

Depois do gate, timeout, excesso de captura, falha de comunicação, falha de
marcador ou morte de uma autoridade seguem: registrar motivo, publicar
`terminating` quando possível, TERM, espera limitada, KILL, reaping do launcher,
reaping do watchdog, marcador terminal e limpeza. Se a ausência de processos
não puder ser provada, o sandbox permanece preservado. `Drop` é apenas a última
defesa conservadora.

## Marcador schema 2

`owner.marker` possui exatamente os campos `schema`, `owner_pid`,
`owner_start_time`, `execution_device`, `execution_inode`, `launcher_pid`,
`launcher_start_time`, `guest_pid`, `process_group_id`, `watchdog_pid`,
`created_at_unix`, `git_head`, `executable_sha256` e `state`. Campos ausentes,
extras, duplicados ou inválidos preservam a entrada. O owner do nome
`exec-OWNER-ID`, device e inode precisam coincidir com o conteúdo.

Os estados são `preparing`, `launcher-ready`, `watchdog-ready`, `running`,
`terminating`, `finished` e `failed`; cada estado valida uma combinação exata
das identidades disponíveis. Schema 1 é legado e recebe `PRESERVED
legacy-marker`: não existe remoção automática durante a migração.

A escrita cria temporário exclusivo no próprio sandbox, escreve tudo, executa
`flush` e `sync_all`, e só então renomeia atomicamente para `owner.marker`.
Symlink no destino é rejeitado. Interrupção conserva o marcador anterior ou
deixa ausência/temporário que o scavenger não interpreta como válido. Os
vetores canônicos em `tests/fixtures/native_marker_vectors.tsv` são consumidos
pelas provas dos parsers Rust e Bash.

## Cleanup e recuperação

`scripts/pinker-cleanup.sh` é dry-run por padrão e exige `--apply`. Ele valida a
raiz canônica e sua identidade, examina apenas nomes `exec-PID-ID`, exige schema
2 completo, idade mínima e owner comprovadamente inativo. A saída distingue
`STALE`, `PRESERVED` e `ERROR` com o mesmo veredicto do scavenger Rust.

No modo apply, a entrada é renomeada sob a mesma raiz para nome privado
`.pinker-quarantine-*` por operação no-replace. Device e inode do nome de
quarentena são comparados com a identidade registrada; só o objeto movido e
confirmado é removido. O caminho `exec-*` original nunca é removido depois da
quarentena. Colisão, troca antes do rename ou troca depois dele resultam em
`PRESERVED`, mantendo o objeto divergente para inspeção.

A ferramenta é idempotente, não segue symlink e não remove conteúdo externo.
Marcador ausente, truncado, duplicado, incompleto, legado ou symlink é
preservado. Falha de parser de `/proc` também é preservada. A tentativa esperada
de abrir `/proc/PID/stat` ausente silencia apenas esse erro; `/proc` ausente ou
PID existente com conteúdo ilegível continuam classificados como `Unknown`.

## Proveniência e diagnóstico

Em falhas, a autoridade registra caso lógico, Git HEAD real, SHA-256 do
executável, SHA-256 do runtime quando aplicável, PIDs do launcher, convidado e
watchdog, PGID,
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
