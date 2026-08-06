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

Cada processo recebe uma allowlist explícita de descritores. O controlador
captura a tabela de FDs antes do fork do launcher sob exclusão mútua apenas da
janela global `snapshot -> fork -> LauncherReady`; essa trava não abrange o
watchdog, o gate, o convidado nem a execução da suíte. Antes do `exec`, o
launcher conserva somente stdio, status, gate, controle e lifecycle; depois do
fork do convidado conserva somente status, controle e lifecycle; o watchdog
conserva apenas seus três canais autorizados. O convidado fecha controle e todo
FD herdado gravável ou sem `CLOEXEC`, preservando apenas stdio e o pipe interno
de erro de `std::process` até o `exec`. A varredura pré-exec usa primitivas raw
em `/proc/self/fd`, sem alocação depois do fork. Assim uma execução concorrente
não pode manter aberto para escrita o executável de outra execução.

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

### Varredura de descendentes e prova de ausência

A autoridade sobre a árvore não mantém tabela de processos. `/proc` é enumerado
de forma incremental: cada PID é classificado e, quando exigido, sinalizado
antes de a próxima entrada ser lida, e nenhuma entrada é retida depois disso. O
espaço é constante por candidato. Os únicos arranjos fixos são buffers de
leitura de `dirent` e de `stat`; **4096 e 8192 são tamanhos de buffer de I/O, não
limites de quantidade de processos**. Não existe teto de quantidade nem teto de
altura da cadeia de ancestralidade.

A cadeia é percorrida de forma iterativa, jamais recursiva. Sequências
patológicas são detectadas por cursores lento e rápido sobre a mesma cadeia, em
espaço constante.

O campo `ppid` é classificado por uma autoridade única, que distingue:

- `pid`: inteiro estritamente positivo;
- `ppid`: inteiro **não negativo**;
- `ppid` zero: fronteira raiz do kernel, não descendente do launcher;
- `ppid` um: raiz de userspace, não descendente do launcher;
- `ppid` igual ao próprio `pid`: ciclo declarado pela entrada;
- campo ausente, não numérico, com sinal ou `stat` truncado: parse inválido;
- processo desaparecido entre a enumeração e a leitura: desaparecido;
- erro de leitura: erro classificado, distinto de desaparecido.

`ppid` zero é representação válida de fronteira raiz. Confundi-lo com entrada
malformada transformava `kthreadd` e toda a sua descendência em incógnita
permanente, e nenhuma varredura voltava a provar ausência. Entradas com `ppid`
zero não impedem a prova de ausência de descendentes.

A sinalização fixa a identidade do alvo com `pidfd` **antes** do sinal e
revalida a ancestralidade **depois** de fixá-la, de modo que um PID reutilizado
por processo alheio à árvore nunca recebe sinal. Todos os descritores abertos
na varredura são fechados no mesmo caminho.

A ausência só é afirmada após uma varredura **completa** e sem incógnitas.
Passagem interrompida no meio do diretório não prova nada: o descendente
sobrevivente poderia estar exatamente na parte não lida. Identidade desconhecida
jamais é convertida em prova de encerramento, e a prova vale apenas para a
passagem que a produziu — uma passagem posterior pode encontrar processo criado
depois da anterior.

Erro em uma entrada nunca impede o tratamento das demais: a entrada vira
incógnita tipada e um descendente comprovado depois dela ainda recebe o sinal.
Falha de varredura na fase TERM também não encerra a supervisão antes do KILL;
o erro é retido como causa secundária e a causa primária permanece tipada como
árvore sobrevivente ao prazo absoluto.

A enumeração de candidatos, a leitura de identidade, a resolução de
ancestralidade e a decisão de sinalização são parâmetros genéricos da mesma
função de decisão, resolvidos por monomorfização. Não há objeto de trait, não há
despacho dinâmico e não há alocação no caminho pós-fork. As regressões
sintéticas atravessam exatamente essa autoridade: nenhuma lógica de
ancestralidade é duplicada em implementação exclusiva de teste. Elas cobrem zero,
um, 4095, 4096, 4097, 8192, 8193 e 20000 candidatos, descendente no primeiro, no
último e em posições separadas, cadeia de 12000 elos, término em `ppid` zero e
em `ppid` um, pai ausente, desaparecimento de candidato e de ancestral, PID
reutilizado, identidade divergente, auto-pai, ciclo de dois nós e ciclo maior,
`stat` truncado, PID inválido, `ppid` inválido e erro de leitura. Quantidades
acima de milhares vêm de fonte sintética; nenhuma delas cria processo real.

### Causa tipada, precedência e shutdown composto

A autoridade interna usa `TerminationReason`, nunca texto livre. Uma função
pura recebe todos os eventos observados na mesma iteração e aplica a ordem
fechada: watchdog morto, controlador perdido, launcher falho, stdout excedido,
stderr excedido, timeout, falha de startup e saída normal do convidado. Se uma
causa já estiver fixada, a função a devolve sem alteração: a primeira causa
vence inclusive quando outro limite se torna verdadeiro durante o shutdown.

`ControlledRunOutcome` conserva status, causa primária, todos os
`ShutdownError` secundários com estágio e identidade aplicável, identidades do
launcher/watchdog, prova da morte da árvore e disposição removida ou preservada
do sandbox. TERM, espera, KILL, reap, finalização do watchdog, marker terminal,
quarentena, cleanup e evidência acumulam falhas sem retorno antecipado que
apague a causa. `output()` e `status()` preservam a API compatível e só
renderizam o relatório textual nessa fronteira.

### Lifecycle estruturado e identidade direta do watchdog

`LifecycleProbe` mantém registros tipados com sequência monotônica sob mutex e
condvar. O canal de reexecução usa um socket Unix exclusivo com timeout:
`LauncherReady`, `WatchdogReady`, `GuestStarted`, `SandboxRunning`,
`WatchdogExitObserved`,
`PrimaryReasonLatched`, TERM/KILL, reaps, disposição do sandbox e publicação do
resultado chegam diretamente ao teste. O watchdog só recebe `SIGKILL` depois
que PID e start time correntes em `/proc` coincidem com a identidade publicada.
O marker continua evidência secundária; nenhuma varredura de
`target/pinker-exec` descobre o processo.

As fixtures de árvore publicam PIDs e start times do filho e do neto pelo mesmo
canal direto antes de liberar o teste. A troca controlada da raiz aguarda
`SandboxRunning`, em vez de disputar a transição do marcador. Todas as esperas
do teste de morte do watchdog compartilham um prazo operacional absoluto de 60
segundos e, ao expirar, mostram o último evento e o journal completo; não há
polling de strings nem uma janela local artificial de 5 segundos.

O outcome entre processos é escrito em temporário exclusivo, sincronizado e
publicado por rename antes de `ResultPublished`. Assim, existência de arquivo
jamais significa conteúdo parcial. Esperas bloqueantes limitadas preservam o
journal ordenado ao expirar.

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

`scripts/pinker-flake-runner.sh` executa lotes finitos. Sucessos removem stdout,
stderr, journal e amostras; falhas preservam diretório exclusivo sob
`target/pinker-flake-evidence/` com iteração, filtro, threads, duração, head,
teste falho, identidades disponíveis, markers/resultados, eventos, snapshots
`/proc`, processos, árvore do sandbox e amostras de processos/sandboxes.

### Falso verde do runner de estabilidade

O runner tratava o código de saída do harness como suficiente. Três formas de
verde falso passavam:

- **exit status booleano**: a contagem de falhas era usada diretamente como
  código de saída, e o shell a trunca em módulo 256, de modo que exatamente 256
  falhas produziriam zero, isto é, sucesso aparente. Os códigos passam a ser
  fixos e disjuntos: `0` sucesso, `1` falhas, `2` erro de uso, `130`
  interrompido;
- **zero testes executados**: um lote cujo filtro não casava com teste algum
  terminava em zero sem executar nada e era contado como iteração bem-sucedida.
  Uma iteração só é `PASS` quando o harness termina em zero, existe resumo
  reconhecido pertencente àquela execução e pelo menos um teste foi de fato
  executado. Testes ignorados e filtrados não contam como executados;
- **resumo inválido**: saída sem linha de resumo reconhecível era aceita. Agora
  falha fechada com `unparseable-test-summary` e preserva evidência.

Nenhuma execução comprovada nunca produz resumo verde: um lote que completa zero
iterações é contabilizado como falha. A validação de uso ocorre antes de criar
diretório de evidência, antes de apagar resumo anterior e antes de iniciar
qualquer teste, de modo que erro de uso não altera o estado de um lote anterior.

Interrupção é preservada, não apagada: o `trap` encerra o controlador `setsid`
pela identidade revalidada, promove o diretório em curso a `INTERRUPTED-*` e
sai com `130`. Uma iteração interrompida nunca é contada como concluída.

### Janela de interrupção ao iniciar a campanha

A janela abaixo foi **descoberta durante a PR #424** e deliberadamente deixada
fora do escopo daquela unidade, que tratava da publicação segura de
executáveis. Ela é corrigida nesta unidade posterior. Nada do que a PR #424
entregou é reescrito aqui.

**A janela antiga.** O runner criava o diretório `.running-*`, iniciava
`setsid timeout ...` em segundo plano, capturava `$!` como controlador e só
então lia `/proc/<pid>/stat` repetidamente para descobrir o `start time`. Os
`trap` de `INT`, `TERM` e `HUP` já estavam ativos durante toda essa sequência, e
o encerramento começava por uma validação que exigia simultaneamente PID
presente, start time presente, processo existente e grupo correspondente.

Um sinal recebido **depois** da criação do controlador e **antes** da captura do
start time encontrava essa validação falhando. O encerramento retornava sem
encerrar coisa alguma, e o runner ainda assim saía com `130`, liberava o lock e
preservava a evidência como interrompida — deixando `timeout`, harness,
descendentes e o subshell monitor vivos. Interrupção correta por fora, árvore
viva por dentro: o modo de falha mais caro, porque nada no resultado denuncia o
resíduo.

**Estados explícitos.** O ciclo do controlador deixa de ser inferido da
combinação de variáveis vazias e passa a ser um estado nomeado:

| estado | significado |
|---|---|
| `idle` | nenhum controlador iniciado nesta iteração |
| `starting` | criação iniciada, identidade ainda não confirmada |
| `active` | PID, start time, PGID e SID capturados e validados |
| `reaping` | encerramento e `wait` em andamento |
| `finished` | filho aguardado e estado limpo |

A fase `starting` é instalada **antes** da linha que cria o controlador e só
termina depois de a identidade estar completa, revalidada e promovida.

**Interrupção pendente e primeira causa imutável.** Durante `starting`, `INT`,
`TERM` e `HUP` são registrados como interrupção pendente e o handler retorna: ele
não executa encerramento com identidade incompleta, não sai do runner e não
ignora o sinal em definitivo. A primeira causa recebida vence e nunca é
substituída por um sinal posterior; um segundo sinal durante o encerramento não
reinicia a escalada. A causa, o estado em que ela chegou, a existência do
controlador naquele instante e o resultado do encerramento são registrados no
manifesto da evidência interrompida, que continua sendo um conjunto de linhas
`chave=valor`.

A ordem entre sinais simultaneamente pendentes é do kernel, não de quem envia —
no Linux vence o menor número. "Primeira causa" significa, portanto, a primeira
**recebida**, e as regressões a estabelecem por barreira explícita em pontos
distintos da inicialização, nunca por ordem de envio.

**Identidade completa, anunciada pelo próprio controlador.** A identidade ativa
exige quatro campos: PID, start time, PGID e SID. Ela não é mais garimpada em
`/proc` pelo runner: o controlador a publica em um canal exclusivo do lote antes
de executar o harness, e só então entrega o processo por `exec`, que preserva os
quatro campos. A razão é medida, não suposta — o Bash colhe filhos de segundo
plano por conta própria, e `/proc/<pid>` de um harness rápido desaparece antes
que o runner consiga lê-lo. Uma identidade anunciada não corre com a morte de
quem a anunciou.

O anúncio é validado estritamente: marca fechada, versão fechada, quatro campos
decimais e nada além. Exige-se que ele pertença ao filho direto da iteração, que
o controlador lidere o próprio grupo e a própria sessão, que nem o grupo nem a
sessão sejam os do runner, e que `/proc` concorde com o anúncio enquanto o
processo existir. `setsid` não cria processo intermediário nesta máquina, e a
igualdade `pid = pgid = sid` é exigida por medição, não presumida por
coincidência.

**Falha fechada quando a identidade não pode ser provada.** O runner não segue
com identidade incompleta. Não inicia o monitor, não executa as iterações
seguintes, não publica `PASS` e não publica resumo: contém o filho direto,
aguarda-o, preserva evidência diagnóstica em `IDENTITY-FAILURE-*`, libera o lock
e termina com o código `4`, distinto de falha de teste e de lock.

**Autoridade de sinal.** A contenção primária é o filho direto, pelo PID: um
filho ainda não aguardado não pode ter o número reutilizado, e `timeout` propaga
o sinal ao grupo que administra. O sinal **coletivo** exige identidade
revalidada imediatamente antes — PID, start time, PGID e SID —, porque um PGID
pode ter deixado de existir e passado a nomear outro grupo. Identidade
desconhecida nunca autoriza sinalização coletiva. Nenhum processo é alcançado
por nome, por substring ou por PID sem start time.

**Contenção do monitor.** O subshell de amostragem pertence ao runner tanto
quanto o controlador. Ele é inicializado explicitamente, sai sozinho ao observar
a morte do controlador — observável apenas depois do `wait` do filho direto — e
é sempre aguardado. A evidência só é movida depois disso, de modo que o monitor
nunca escreve num diretório já promovido a `INTERRUPTED-*` nem é reparentado
para o init. A contenção por PID é limitada e só age quando a saída espontânea
não acontece: derrubar o subshell no meio de uma amostragem deixaria órfãos os
processos que ele acabou de criar.

**Ordem do encerramento.** Congelar a primeira causa; impedir reentrância;
completar ou validar a identidade; revalidar os quatro campos; `TERM` na árvore
autorizada; prazo limitado; revalidar; `KILL` somente nos sobreviventes
autorizados; aguardar o filho direto; encerrar ou aguardar o monitor; confirmar
ausência de descendentes; promover `.running-*` a `INTERRUPTED-*`; preservar
manifesto e evidência; liberar o lock pelo caminho canônico; sair com `130`.

O código continua sendo `130` para `INT`, `TERM` e `HUP`, o lote interrompido
continua preservado, a última projeção de lote concluído permanece intacta e
nenhum resumo novo é publicado.

**Interrupção normal e `SIGKILL` são contratos distintos.** Uma interrupção
normal executa os traps: o runner encerra a própria árvore, aguarda o filho
direto e o monitor, libera o lock e não deixa resíduo. `SIGKILL` não executa
trap algum: o lock sobrevive carregando identidade suficiente para que uma
campanha posterior o classifique e o recupere, e um `.running-*` deixado por
`SIGKILL` pertence a esse outro contrato — não é resíduo inexplicado e não é
removido automaticamente.

**Gancho determinístico.** Um gancho estritamente de teste congela a
inicialização em `before-spawn`, `after-spawn`, `after-identity`, `after-active`
e `after-monitor`. Ele é inativo por padrão e exige duas variáveis — o programa
e a lista explícita de estágios —, de modo que configuração parcial não ativa
comportamento algum. É deliberadamente separado do gancho de lock já existente,
que continua recebendo apenas `before-lock-removal`: ampliar aquele faria o seu
único consumidor agir em pontos que nunca esperou.

### Exclusividade por checkout

Proibição operacional não é correção. Duas campanhas sobre o mesmo `target`
compartilhavam progresso, resumo, sandboxes, markers e arquivos auxiliares, e o
runner removia `PROGRESS-<mode>.txt` e `SUMMARY-<mode>.txt` no início de cada
lote: a segunda destruía a evidência da primeira e podia terminar verde
escondendo as iterações falhadas da outra. A autoridade passa a **impedir
tecnicamente** a concorrência.

Somente uma instância do runner opera sobre o `target` de um checkout por vez,
independentemente de mode, filtro ou número de threads. A aquisição é um
`mkdir` de `target/pinker-flake-evidence/.lock`, atômico em POSIX e sem
depender de `flock`. O `flock` existe na máquina e não é usado de propósito:
seu lock vive preso a um descritor aberto e desaparece junto com o processo,
inclusive sob `SIGKILL`, enquanto este contrato exige exatamente o oposto — que
o lock sobreviva ao dono morto carregando a identidade que permite
classificá-lo. O lock carrega um marker de campos
fechados e ordenados: `schema`, `runner_pid`, `runner_start_time`, `mode`,
`head_git`, `created_at_unix` e `batch_id`. Linha faltando, linha sobrando,
chave fora de ordem, chave repetida ou valor fora do domínio tornam o marker
inválido, e marker inválido falha fechado.

Diante de um lock existente, a decisão nunca vem do nome. O runner exige
diretório real — symlink é recusado —, valida o marker estritamente e
classifica a identidade do proprietário pela mesma autoridade usada no resto da
contenção:

- `live`: PID existe e o start time confere. A segunda campanha é rejeitada com
  saída não zero e diagnóstico determinístico, **antes** de remover resumo,
  criar progresso, iniciar teste ou tocar `target/pinker-exec`. Não há espera
  silenciosa, não há retry automático e a campanha proprietária não é
  sinalizada;
- `missing` ou `reused`: prova positiva de que a campanha proprietária
  terminou. O lock obsoleto é recuperado por transação que revalida o marker
  imediatamente antes de removê-lo; identidade divergente nesse intervalo
  preserva o lock;
- `unknown`: falha fechada e preservação. Identidade não provada jamais
  autoriza remoção.

O lock é liberado em sucesso, falha comum, erro posterior à aquisição, `SIGINT`,
`SIGTERM` e `SIGHUP`. Antes de liberar, o runner revalida diretório, marker e
identidade, e remove somente o lock desta instância. `SIGKILL` não executa trap
algum: é justamente por isso que o marker registra identidade suficiente para
que uma campanha posterior classifique o lock deixado para trás e o recupere.

### Namespace de lote e projeções legadas

Cada campanha possui identificador próprio e diretório exclusivo em
`target/pinker-flake-evidence/batches/<batch-id>/`, contendo manifesto,
progresso, resumo e as evidências das iterações. **A autoridade do resultado é
esse diretório.**

`PROGRESS-<mode>.txt` e `SUMMARY-<mode>.txt` deixam de ser autoridade e passam
a ser projeção do último lote concluído. Nunca são removidos no início; são
publicados apenas ao final, por temporário e rename atômico, e carregam
`batch_id`, `head_sha`, `authority` e `projection=last-completed-batch`. Um
leitor nunca observa arquivo parcial.

Disso decorrem duas garantias que o incidente violava: uma campanha
interrompida não chega à publicação e portanto **não substitui o último resumo
completo**; e uma campanha falhada publica o resumo falhado do próprio lote,
com `failures` maior que zero, sem poder herdar o verde de outra.

### Recuperação de quarentena

Uma quarentena interrompida deixava diretório órfão sem autoridade que o
retomasse. As duas autoridades — a nativa e `scripts/pinker-cleanup.sh` —
passam a reconhecer e recuperar quarentenas incompletas, preservando colisão de
nome em vez de sobrescrever evidência. Sucesso não preserva sandbox; falha
preserva.

## Gate de estabilidade desta correção

O baseline anterior à mudança completou 30/30 arquivos sequenciais (690
testes), 30/30 arquivos com threads padrão (690 testes), 50/50 execuções do
teste conhecido e 30/30 grupos launcher/watchdog (420 testes), sem reproduzir
as duas falhas históricas. Uma iteração interrompida não foi contada e sua
evidência permaneceu preservada.

Sobre os bytes finais, todos os lotes obrigatórios terminaram sem falha:

- teste conhecido, um thread: 100/100, 100 testes, 382.795 ms, máximo de 5
  processos e 3 sandboxes;
- teste conhecido, harness normal: 100/100, 100 testes, 382.364 ms, máximo de
  5 processos e 3 sandboxes;
- arquivo completo, um thread: 100/100, 2.800 testes, 2.420.642 ms, máximo de
  5 processos e 4 sandboxes;
- arquivo completo, threads padrão: 100/100, 2.800 testes, 746.680 ms, máximo
  de 26 processos e 8 sandboxes;
- grupo launcher/watchdog, threads padrão: 100/100, 1.900 testes, 530.691 ms,
  máximo de 25 processos e 10 sandboxes.

Esses cinco lotes reutilizaram o mesmo binário compilado depois da última
alteração de código: 500 repetições, 7.800 testes e zero falhas. Execuções
anteriores à última correção não foram contadas como evidência do head final.

A falha conhecida combinava sobrescrita de `WatchdogExit` por um limite
observado na mesma iteração e retornos antecipados capazes de perder a causa
durante o shutdown. A causa tipada, a seleção pura com precedência explícita,
o primeiro latch imutável e a retenção de erros secundários corrigem esse
mecanismo. O teste deixou de descobrir watchdog por marker e passou a verificar
PID e start time recebidos pelo canal lifecycle antes do sinal.

A segunda falha histórica não possuía nome de teste nem evidência preservada,
mas a execução completa sob carga identificou manifestações concretas além da
falha conhecida: `supervisor_fecha_descritores_gravaveis_herdados` produziu
`ETXTBSY`, e `execution_root_symlink_e_entradas_symlink_nunca_escapam` junto de
`raiz_real_ausente_existente_e_segunda_execucao_sao_idempotentes` atingiram um
timeout artificial de fixture. A inspeção também encontrou uma janela distinta
e determinística na publicação do outcome: `fs::write` tornava o arquivo visível
antes de completar os bytes. A regressão
`publicacao_atomica_elimina_janela_de_resultado_parcial` reproduz a janela
antiga por canais e prova temporário exclusivo, `sync_all` e `rename` na
publicação nova. Uma recorrência futura agora preserva resultado, eventos,
identidades e arquivos auxiliares automaticamente.

No `ETXTBSY`, a prova abria a cópia executável para escrita e o launcher fechava
FDs não relacionados somente depois de criar o convidado. Um fork concorrente
podia herdar aquele descritor gravável e mantê-lo vivo até seu próprio `exec`,
bloqueando o `exec` do proprietário. A correção é a allowlist por processo e a
exclusão estreita da janela global de snapshot/fork descrita acima. A regressão
isolada `supervisor_fecha_descritores_gravaveis_herdados` prova que launcher,
watchdog e convidado fecham o descritor; `watchdog_fd_allowlist_probe` verifica
diretamente `/proc/PID/fd/FD` no watchdog identificado.

Nos dois testes de sandbox, `FakeRepo::controlled` impunha um override local de
3 segundos sem relação com o contrato das asserções. Sob carga, a autoridade
selecionava corretamente `Timeout` embora o convidado terminasse em seguida. O
override foi removido: a fixture usa a política operacional `Common` de 20
segundos e aguarda `GuestStarted`/resultado pelo canal, sem sleep. A regressão
`fixture_de_sandbox_preserva_timeout_operacional` comprova a política e a saída
estruturada `GuestExited`. Nenhum lock global foi adicionado. As falhas estão
preservadas em
`20260802T044209.579283283Z-stability-complete-parallel-default-1-1865290` e
`20260802T054910.868032911Z-stability-complete-parallel-default-72-1873760`.

Uma validação intermediária do grupo revelou ainda que o teste conhecido
mantinha esperas locais de 5 segundos para `LauncherReady`/`WatchdogReady`,
apesar do contrato operacional de 60 segundos. Sob disputa legítima da janela
estreita de fork, o último evento permanecia vazio e a fixture expirava antes
do controlador. Todas as etapas agora derivam do mesmo prazo absoluto de 60
segundos, com diagnóstico do journal no timeout; a regressão integrada do grupo
falhou deterministicamente antes da correção e completou 100/100 depois dela.
Sucessos dos lotes finais não deixaram diretórios de evidência.

### Gate da segunda unidade

A campanha do head intermediário não é gate. Duas campanhas pesadas chegaram a
executar simultaneamente sobre o mesmo clone, compartilhando
`target/pinker-flake-evidence` e `target/pinker-exec`; como o runner apaga
`PROGRESS-<modo>.txt` e `SUMMARY-<modo>.txt` no início de cada lote, um resumo
sobrescreveu o outro e três iterações falhadas ficaram invisíveis atrás de um
`failures=0`. Todo resultado daquela execução está rotulado `diagnostic_only:
true` e não é atribuído a head algum. Campanhas pesadas não devem executar
simultaneamente sobre o mesmo diretório de trabalho.

A campanha final executou sobre o head de código final, com binário único e
sem recompilação entre grupos, e os quatro grupos correram sequencialmente
entre si:

- 50 focadas sequenciais: 50/50, 1.800 testes, 382.683 ms, máximo de 5
  processos e 2 sandboxes;
- 50 focadas com threads padrão: 50/50, 1.800 testes, 164.720 ms, máximo de 10
  processos e 3 sandboxes;
- arquivo completo, um thread: 20/20, 900 testes, 301.328 ms, máximo de 5
  processos e 2 sandboxes;
- arquivo completo, threads padrão: 20/20, 900 testes, 92.598 ms, máximo de 10
  processos e 2 sandboxes.

São 140 repetições e 5.400 testes com zero falhas. Ao final de cada grupo e ao
final da campanha: zero processos residuais, zero sandboxes residuais e nenhum
diretório de evidência — sucesso não preserva evidência indevida. O teste de
sensibilidade passou e as fontes foram restauradas byte a byte, com árvore
limpa e delta temporário vazio.

Essa campanha valida a **contenção**, cujo código não mudou desde então. A
exclusividade por checkout e o namespace de lote pertencem ao runner de
evidência e são validados por uma campanha representativa curta, executada com
o runner novo sobre o mesmo binário de teste:

- 5 focadas sequenciais: 5/5, 180 testes, 87.970 ms;
- 5 focadas com threads padrão: 5/5, 180 testes, 38.359 ms;
- arquivo completo, um thread: 1/1, 45 testes, 23.962 ms;
- arquivo completo, threads padrão: 1/1, 45 testes, 7.926 ms.

Cada grupo produziu o próprio lote, com manifesto e resumo exclusivos, e
liberou o lock ao terminar; zero processos e zero sandboxes residuais. Com uma
campanha proprietária em andamento sobre o checkout real, uma segunda campanha
foi rejeitada com saída `3` e diagnóstico nomeando `owner_pid`,
`owner_start_time` e `identity=live`, sem tocar a projeção do lote anterior e
sem afetar a campanha proprietária, que seguiu até o fim com saída zero.

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
