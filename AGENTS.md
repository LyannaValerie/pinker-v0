# AGENTS.md

Guia operacional curto para agentes neste repositório. Não substitui `README.md`, `MANUAL.md` ou docs canônicos de `docs/`. Esta é a **fonte operacional canônica** para agentes; não crie um segundo contrato (`CLAUDE.md` só deve existir se uma integração o exigir fisicamente).

## Bootstrap de Task (antes de qualquer Cargo)

Em Task da Forja, execute este bootstrap **antes do primeiro `cargo`, `make` ou
`pink` da árvore**. Nenhum comando normativo deste arquivo deve rodar Cargo antes
dele.

### A memória do agente não é autoridade

```text
AGENT_PERSISTENT_MEMORY = UNRELIABLE
AGENT_PERSISTENT_MEMORY_IS_AUTHORITY = FALSE
```

Sua memória pode sugerir onde procurar. Ela não prova baseline, caminho,
comando, estado da Task, decisão da Founder nem arquitetura corrente. Autoridade
é o que se observa agora: Issue/PR, checkpoint, Book, Trama, código, testes,
`git`, inventário do host e os observadores da Forja. Um caminho lembrado de
outra Task é hipótese, não fato — confirme antes de usar.

### Topologia corrente

```text
CANONICAL_MAIN     /pinker/repo/pinker-v0
AGENTES_ROOT       /pinker/repo/pinker-v0/agentes
TASK_ROOT          /pinker/repo/pinker-v0/agentes/<slot>    (observado)
```

O checkout canônico é o **integrador mãe**. Ele acompanha `origin/main`, fica
limpo e **não recebe mutação de implementação de Task**:

```text
TASK_IMPLEMENTATION_MUTATES_CANONICAL_MAIN_CHECKOUT = FALSE
```

Toda Task trabalha na própria worktree, dentro do próprio root. Uma Task tem UM
root físico, e todo recurso descartável exclusivo dela mora lá dentro.

**Fase 0 — sem build.** Recupere identidade e ambiente; ainda não compile.

1. Recupere a Task e o active context. O `TASK_ID` vem do observador canônico, não
   do prompt nem de um nome inventado:

   ```bash
   sudo -n /opt/pinker/bin/forja-lifecycle show
   ```

2. Se precisar de diagnóstico de ambiente antes de ter árvore construída, use o
   `pink` instalado — e **somente** para identidade e compatibilidade:

   ```bash
   command -v pink
   pink --version-json
   pink doctor
   ```

   O binário instalado é autoridade para "quem sou eu?" e "reconheço este
   repositório?". Ele **não** é autoridade sobre catálogo, receita, projeção ou
   subcomando que tenha mudado no `HEAD` corrente.

3. Provisione e **observe** o root físico. O slot não deriva do `TASK_ID`:
   descubra-o, não o construa.

   ```bash
   forja-agentes provision --branch <branch-da-task>   # idempotente
   forja-agentes observe                               # layout em JSON
   eval "$(forja-agentes env)"                         # exports do layout
   ```

   `env` exporta exatamente o contrato — e nada fora do root:

   ```text
   TASK_ID  FORJA_TASK_ROOT  FORJA_CANONICAL_MAIN
   TASKDIR            <TASK_ROOT>/worktree
   CARGO_TARGET_DIR   <TASK_ROOT>/target
   TMPDIR             <TASK_ROOT>/tmp
   FORJA_SCRATCH      <TASK_ROOT>/scratch
   FORJA_TASK_MEMORY  <TASK_ROOT>/memory
   FORJA_TASK_STATE   <TASK_ROOT>/state
   FORJA_TASK_ARTIFACTS <TASK_ROOT>/artifacts
   ```

   Nunca monte o root por concatenação:

   ```text
   TASK_IDENTITY != PHYSICAL_PATH
   AGENTES_ROOT + "/" + TASK_ID  ->  ERRADO
   ```

   Dois detalhes que quebram o bloco acima se ignorados:

   - **Use caminho absoluto no `sudo`.** O `secure_path` do sudoers não inclui
     `/opt/pinker/bin`, e a regra é concedida por caminho absoluto. `sudo -n
     forja-lifecycle show` falha com `command not found`, e dentro de uma
     substituição de comando esse erro é engolido: `TASK_ID` sai vazio e o
     `export` seguinte não reclama. Confira que `TASK_ID` não está vazio.
   - **Confirme cobertura antes de construir.** `forja-agentes observe` lista os
     recursos com `present: true`. O que não aparece ali não é provisionado nem
     finalizado — é lacuna de cobertura a resolver **antes** do primeiro build,
     não depois.

4. Verifique os invariantes do layout quando algo parecer estranho:

   ```bash
   forja-agentes verify     # exit 0 = íntegro; exit 5 = invariante violado
   forja-agentes list       # todos os slots observados
   ```

**Fase 1 — com build.** Só agora construa, sempre dentro da worktree da Task, e
use o `pink` da árvore para tudo que dependa do estado corrente do repositório:
`doc`, `nav`, `impacto`, `verificar`, sincronização e manutenção de projeção.

### Memória factual da Task

A memória operacional da Task vive em `<TASK_ROOT>/memory` e é **estruturada**:

```text
AGENT_MEMORY_FORMAT = JSON_OR_JSONL
```

Markdown não é memória operacional primária. Formato usual:

```text
state.json        identidade, baseline, SHAs, estágio
events.jsonl      fatos datados: probes, decisões, Book reads
findings.jsonl    achados com evidência
adversarial.jsonl produzido pelo agente adversarial, nunca pelo PRIMARY
```

Registre apenas fato compacto: SHA, caminho observado, id de recurso, comando
resumido, resultado, decisão, hipótese marcada como tal, ponteiro de evidência,
limitação. Nunca chain-of-thought, transcript integral, segredo, credencial ou
diário narrativo.

### Ciclo de vida da Task: o que você precisa saber para operar

O contrato é de invocação, não de implementação. A Forja é a autoridade sobre o
próprio layout, o próprio modelo de ameaça e a própria semântica de destruição;
este arquivo só diz o suficiente para você usá-la corretamente.

```text
EXECUTION_SEAL   após implementação/testes/PR, quando seguro
                 recupera o efêmero (target, cache, tmp, scratch, logs)
                 preserva worktree, branch/HEAD, memory, state, artifacts

TASK_RETIRE      somente após merge humano + main verde + decisão do Guia
                 remove a subárvore inteira do Task root

ACTIVE ──> REVIEW ──> FIX_REQUIRED ──> SEALED ──> RETIREABLE
ACTIVE ──> RETIREABLE                             PROIBIDO
```

```bash
forja-agentes seal                 # plano, não remove nada
forja-agentes seal --apply         # recupera o efêmero; deixa a Task SEALED
forja-agentes state --set RETIREABLE
forja-agentes retire               # plano com as provas exigidas
forja-agentes retire --apply       # destrói, fail-closed
```

Três consequências práticas, e o resto é da Forja:

- **Não retire antes do merge.** A revisão, a verificação do PRIMARY e a
  disposição do Guia ainda precisam da worktree e da memória.
- **Comando mutante age sobre a própria Task.** Em `state`, `seal` e `retire`,
  `--task-id` é asserção, não endereço: divergir do que o observador canônico
  atribui ao chamador é recusado.
- **A destruição é fail-closed e diz o alcance da própria prova.** Quando a
  saída trouxer `uninspectable_*`, ela está declarando o limite da evidência em
  vez de escondê-lo; leia antes de concluir qualquer coisa sobre resíduo.

Por que `CARGO_TARGET_DIR` sai no slot `target` do Task root: dentro da worktree
ele seria `DURABLE` e o selo preservaria gigabytes de build sem razão. O
`forja-agentes env` já resolve isso — use o que ele exporta.

Autoridade da implementação (fonte, suítes e instalador) vive no host, não neste
repositório — ver *Onde vive a Forja*.

### Onde vive a Forja

A Forja serve aos agentes que trabalham na Pinker. Ela **não** é produto Pinker,
e este repositório não é autoridade sobre a implementação dela (Issue #544).

```text
PINKER_REPO   produto Pinker + autoridade mínima de integração
FORJA_HOST    autoridade operacional da Forja
```

| o que | onde |
|---|---|
| binários operacionais | `/opt/pinker/bin/` (`forja-agentes`, `forja-lifecycle`, `ls-forja`, `pink`) |
| fonte e suítes da Forja | `/pinker/playground/ferramentas-agente/` |
| catálogo de ferramentas | `ls-forja` sobre `/pinker/state/catalogo` |
| conhecimento operacional | `/book` |
| Task roots | `agentes/<slot>` neste checkout, observados — nunca concatenados |

Descubra a ferramenta antes de inventar uma:

```bash
ls-forja --json
ls-forja find <intenção> --json
ls-forja show <nome> --json
```

O que este repositório mantém sobre a Forja é só isto: o contrato de invocação
acima, o `.gitignore` de `agentes/` que impede material de execução de entrar em
PR, e `forja-paths-check`, que recusa arquivo versionado ensinando um layout
aposentado. A suíte operacional da Forja **não** roda no `make ci` da Pinker; ela
é testada onde a autoridade dela vive:

```bash
cd /pinker/playground/ferramentas-agente
python3 -m unittest discover -s . -p 'test_forja_agentes.py'
python3 -m unittest discover -s . -p 'test_sensibilidade.py'
bash instalar-forja-agentes.sh --check
```

### Raiz de execução nativa e ponte da staticlib

Duas coisas diferentes ancoram em `<repo>/target`, e só uma delas é uma ponte a
criar. Não as confunda.

**1. A raiz de execução nativa é contenção deliberada, não path histórico.**

`tests/common/native_process_sandbox.rs` resolve a raiz assim:

```rust
let repo_root = discover_repo_root()?.canonicalize()?;
let target = repo_root.join("target");       // ignora CARGO_TARGET_DIR por desenho
let root   = target.join("pinker-exec");
if !canonical_root.starts_with(&repo_root) { /* PermissionDenied */ }
```

O sandbox é ancorado à raiz canônica do repositório **de propósito** — é a
autoridade de contenção descrita em `development.native-execution-host-containment`.
Consequências práticas:

- `<worktree>/target/pinker-exec/` será criado mesmo com `CARGO_TARGET_DIR`
  apontado para fora. Isso é contenção, não resíduo, e não deve ser "consertado".
- Trocar `<repo>/target` por symlink para fora da árvore é **rejeitado**: a raiz é
  canonicalizada e precisa continuar contida em `repo_root`.

O sandbox é criado dentro de `ControlledCommand::output()`
(`tests/common/native_process.rs`), então **todo** alvo que use `ControlledCommand`
herda a raiz compartilhada — hoje 40 arquivos de teste, não um punhado:

```bash
git grep -l "ControlledCommand" -- 'tests/*.rs' | wc -l
```

Descubra a lista pelo comando acima; não confie numa enumeração transcrita aqui,
que envelhece a cada Task.

Exceção conhecida: `tests/native_quarantine_recovery_tests.rs` **não** compartilha
a raiz. Ele monta uma raiz única por caso sob `target/pinker-quarantine-recovery/`,
com pid e sequência atômica, justamente para não tocar o repositório real.

Ao investigar falha intermitente nesses alvos, considere interferência no sandbox
antes de concluir regressão. O isolamento entre execuções é por **nome único**
(`exec-{pid}-{seq}`, em `tests/common/native_process_sandbox.rs`), não por
serialização — não procure um lock que não existe. A superfície de corrida é o
diretório-pai `target/pinker-exec/`, na criação, validação e limpeza, e `cargo test`
roda os binários de integração em paralelo sobre ele.

**2. A ponte da staticlib é exigida por um único alvo.**

| alvo | path exigido | precisa de ponte |
|---|---|---|
| `tests/part_d_native_process_tests.rs:114` | `<repo>/target/debug/libpinker_rt.a` | **sim** |
| demais alvos da lista acima | `<repo>/target/pinker-exec/` | não |

Dois lugares resolvem a staticlib a partir do diretório corrente em vez de
`CARGO_TARGET_DIR`, mas só um deles falha:

- `tests/part_d_native_process_tests.rs:114-115` usa `assert!` duro e quebra com
  `staticlib nativa ausente`;
- `tests/common/native_process.rs:1314-1316` devolve `Option` e degrada para
  `None`, sendo usado apenas para hash de proveniência.

Portanto a ponte é exigida por `part_d_native_process_tests` e por mais nada. Se a
sua Task não executa esse alvo, **não crie ponte alguma**.

Quando ele for aplicável, ligue o artefato real ao caminho esperado depois do
primeiro build:

```bash
mkdir -p "$TASKDIR/target/debug"
ln -sfn "$CARGO_TARGET_DIR/debug/libpinker_rt.a" "$TASKDIR/target/debug/libpinker_rt.a"
```

Sem a ponte, os 8 testes de `part_d_native_process_tests` falham com
`staticlib nativa ausente` — falha de fixture, não regressão de código. Confira o
local do panic antes de investigar semântica.

Em ambos os casos o peso do build continua no slot `target` do Task root e
`<worktree>/target`
fica na casa das dezenas de kilobytes. Ele **não** guarda só o symlink: os fixtures
criam suas próprias raízes ali (`pinker-exec/`, `pinker-cleanup-fake/`,
`pinker-host-fixtures/`, `pinker-quarantine-recovery/` e outras, conforme os alvos
que a Task executar). Todas são contenção, não resíduo. Meça com `du -sh target/`
em vez de assumir a lista.

### Pressão de caminho e `SUN_LEN`

O limite útil de `sun_path` é 107 bytes, e o sandbox nativo monta o socket em
`<repo_root>/target/pinker-exec/exec-<pid>-<seq>/arvore/soquete` — cerca de 50
bytes de sufixo. O layout anterior pagava esse orçamento com o nome da Task
(uma raiz irmã por Task, nomeada pelo `TASK_ID`), e um `TASK_ID` descritivo
estourava o
limite: medido em `issue-525`, o socket deu 108 bytes contra 107 úteis e
`part_c_filesystem_adulto_tests` falhava **dentro do fixture**, antes de
qualquer código Pinker executar.

A topologia corrente separa identidade de caminho e resolve isso na origem:

```text
/pinker/repo/pinker-v0/agentes/a01/worktree     = 43 bytes
+ /target/pinker-exec/exec-1234567-99/arvore/soquete
                                                ≈ 93 bytes  (< 107)
```

O slot é curto **porque o `TASK_ID` não precisa caber nele**. Se ainda assim um
fixture falhar por limite de caminho — `AF_UNIX path too long`, `SUN_LEN`,
`path must be shorter than SUN_LEN` —, verifique primeiro onde o panic ocorreu,
depois meça o caminho real em bytes; nunca encurte a identidade da Task para
caber num socket. `TASK_ID` é a chave pela qual active context, observador e
artefatos se encontram; o caminho físico é problema da Forja, não da identidade.

Nota de escopo, para não mandar ninguém caçar fantasma: os binds de socket unix
em `tests/native_process_control_tests.rs` usam caminhos **relativos** sob
`target/`, e um caminho relativo entra em `sun_path` como foi escrito — o
comprimento da worktree não é contabilizado ali. O site que realmente depende do
caminho absoluto é `tests/part_c_filesystem_adulto_tests.rs`.

## Entrada pela Trama Pinker

A documentação é dual: portais Markdown para humanos, catálogos consultáveis para agentes. Para não varrer `docs/` ou `src/` indiscriminadamente:

Os passos abaixo usam o `pink` da árvore e portanto pressupõem o *Bootstrap de
Task* já concluído. Antes dele, use no máximo o `pink` instalado para identidade
e `doctor`.

1. Leia `README.md` e `docs/atlas.md` (o Atlas aponta para territórios).
2. Descubra destinos: `./ci_env.sh cargo run --bin pink -- doc rota "<intenção>"`.
3. Extraia só a seção necessária: `./ci_env.sh cargo run --bin pink -- doc mostrar <id>` (ex.: `rosa.identity`).
4. Antes de abrir arquivos grandes de código: `./ci_env.sh cargo run --bin pink -- nav buscar "<conceito>"` e `... -- nav mostrar <chave>`.
5. Leia o `README.md` local do território antes de alterá-lo.
6. Ao alterar código, mantenha as âncoras `@pinker-nav:start/end`.
7. Ao alterar documentação, mantenha IDs, frontmatter e âncoras `@pinker-doc`.
8. Para PRs posteriores ao marco (#330), mantenha o bloco ` ```pinker-change ` e rode `./ci_env.sh cargo run --bin pink -- doc importar-pr <n> --corpo <arquivo>`; o importador serializa campos textuais livres como escalares YAML canônicos.
9. Regenere catálogos com `./ci_env.sh cargo run --bin pink -- doc sincronizar` e `... -- nav sincronizar`; valide com `make ci` (inclui `docs-check` e `nav-check`).
10. Não invente estado, testes, histórico ou memória; não faça backfill retroativo (PRs ≤ #330 são rejeitados).

### Use o `pink` da árvore, não o `pink` da Forja

O `pink` instalado na Forja (`/opt/pinker/bin/pink`) é um **release congelado**: ele
carrega o commit de quando foi publicado, não o estado presente do repositório.
Confirme com `pink --version-json` — o campo `binary_commit` é o commit do release,
e `pink doctor` reporta `compatibility: COMPATIBLE_ANCESTOR` justamente quando ele
está atrás do `HEAD`.

Em Task neste repositório, **ignore o `pink` da Forja** e use sempre o binário
construído da própria árvore:

```bash
./ci_env.sh cargo run --bin pink -- <subcomando> [args...]
```

Isso vale inclusive para subcomandos que hoje parecem equivalentes: a equivalência
é uma coincidência do commit corrente, não um contrato. Um catálogo, uma receita de
projeção ou uma regra de `doc` que mudou na sua Task só é enxergada pelo binário da
árvore.

O `pink` da Forja permanece útil para uma coisa só: diagnóstico de ambiente fora de
Task (`pink doctor`, `pink --version-json`), quando você ainda não tem árvore
construída.

### A regra do `--`

`cargo run` repassa verbatim tudo que vem depois do **primeiro** `--`. O `pink`, por
sua vez, também trata o primeiro `--` do próprio `argv` como separador — tudo depois
dele vira argumento de runtime de `--run`. Logo:

```bash
# CERTO — subcomando
./ci_env.sh cargo run --bin pink -- nav buscar "receita"

# ERRADO — o pink recebe `--` como primeiro argumento, não vê o subcomando
# e termina com "Uso inválido: nenhum argumento informado." (exit 2)
./ci_env.sh cargo run --bin pink -- -- nav buscar "receita"

# CERTO — dois `--`: o segundo é o separador de argv do programa Pinker
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .
```

O segundo `--` só existe no modo compilador/execução (`pink [OPÇÕES] ARQUIVO -- ARGS`).
Nenhum subcomando leva o `--` extra. Descubra a lista corrente com `pink --help`
em vez de confiar numa transcrição.

Marco documental e política forward-only: `.pinker/doc.toml`. Manifestos versionados: `.pinker/changes/`.

## Comandos padrão

Em Task da Forja, todos os comandos desta seção e da seguinte pressupõem o
*Bootstrap de Task* concluído — em particular `CARGO_TARGET_DIR` já exportado.

```bash
make preflight
make build
make test
make fmt-check
make clippy
make guard
make ci
make run-example EX=examples/principal_valida.pink
make check-example EX=examples/principal_valida.pink
make audit-example EX=examples/principal_valida.pink
make smoke
```

## Comandos sem `make`

```bash
./ci_env.sh --preflight
./ci_env.sh cargo build --locked
./ci_env.sh cargo test --locked
./ci_env.sh cargo fmt --check
./ci_env.sh cargo clippy --all-targets --all-features -- -D warnings
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .
./ci_env.sh cargo run --bin pink -- examples/principal_valida.pink
./ci_env.sh cargo run --bin pink -- --check examples/principal_valida.pink
```

## Contrato operacional da suíte

- Suíte oficial é **stable-only** no toolchain fixado pelo repositório.
- Não depender de nightly nem de `-Z unstable-options`.
- Caminho oficial precisa passar por `./ci_env.sh`, que saneia `RUSTFLAGS` e `CARGO_ENCODED_RUSTFLAGS` e expõe preflight mínimo de diagnóstico.
- `ci_env.sh` **não** define `CARGO_TARGET_DIR`: quem opera uma Task da Forja é
  responsável por resolvê-lo pelo observador e exportá-lo antes do primeiro build,
  conforme *Bootstrap de Task*.
- Em Task, invoque o `pink` da árvore (`./ci_env.sh cargo run --bin pink -- ...`),
  não o release da Forja em `/opt/pinker/bin/pink`.
- Specs delegados do Pink Agent não autorizam executáveis por si: shell exige
  `PINKER_AGENT_ALLOW_SHELL`, programas externos exigem
  `PINKER_AGENT_EXECUTABLE_ALLOWLIST`, e I/O do runner permanece confinado sem
  seguir links simbólicos.

## Disciplina de inspeção (MCP)

- Não conhece conteúdo real do repositório até inspecionar.
- Para afirmação sobre arquivos, símbolos, fases, docs ou histórico, use MCP ou ferramentas locais primeiro.
- Sempre opere como: localizar -> inspecionar -> extrair -> responder.
- Não leia arquivos grandes por inteiro sem necessidade estrita.
- Prefira buscas direcionadas a varreduras amplas.
- Não invente continuidade, histórico ou estado do repositório.
- Trate docs e código como fonte de verdade só após inspeção.

## Mapa rápido do código

- parser/léxico/AST: `src/token.rs`, `src/lexer.rs`, `src/ast.rs`, `src/parser.rs`
- semântica/layout: `src/semantic.rs`, `src/layout.rs`
- IR/CFG/seleção/máquina: `src/ir.rs`, `src/cfg_ir.rs`, `src/instr_select.rs`, `src/abstract_machine.rs`
- validações de pipeline: `src/ir_validate.rs`, `src/cfg_ir_validate.rs`, `src/instr_select_validate.rs`, `src/abstract_machine_validate.rs`
- backends/runtime/CLI: `src/backend_text.rs`, `src/backend_s.rs`, `src/interpreter.rs`, `src/main.rs`
- testes: `tests/parser_tests.rs`, `tests/semantic_tests.rs`, `tests/interpreter_tests.rs`, `tests/backend_s_external_toolchain_tests.rs`

Mapa curto por feature: `docs/code_map.md`.
Índice rápido de exemplos/testes: `docs/examples_index.md`.

## Regras locais de mudança

- Preservar continuidade factual do workspace e trilha ativa.
- Tarefa operacional não abre fase, Doc, FE ou HF.
- Não tocar docs canônicos por inércia: `docs/history.md`, `docs/handoff_codex.md`, `docs/roadmap.md`, `docs/future.md`, `docs/phases.md`.
- Fora do regime temporário de congelamento documental descrito abaixo, mudança funcional real exige evidência em código, testes e docs canônicos apropriados.
- Não reverter mudanças do usuário sem pedido explícito.
- Vulnerabilidades devem usar o relato privado descrito em `SECURITY.md`, nunca Issues públicas.
- Discussions é espaço exploratório e não autoriza roadmap ou implementação.
- A exceção estreita para arquivos de saúde comunitária está em `docs/doc_rules.md`.
- A Trama Pinker V1 está formalmente concluída. Trama Nova, pós-Trama, TUI,
  edição transacional e expansões de orquestração permanecem adiados até
  `Eixo A — linguagem: COMPLETE`. A janela auxiliar de infraestrutura
  determinística da Issue #417 foi encerrada após suas seis capacidades; seu
  inventário e seus limites permanecem históricos em
  `docs/development/janela-infraestrutura-deterministica.md`, sem autorizar
  novas tarefas por analogia. D2 foi restaurada como prioridade após #417, mas a
  Founder autorizou em seguida uma campanha extraordinária de maturação adulta
  que incorpora D2–D12 e outras expansões explícitas antes da retomada ordinária
  do Eixo A. Mudança semântica fora da Issue dessa campanha, novo executor,
  auto-merge ou modificação automática de fontes continuam sem autorização.
- Validar com `build`, `test`, `fmt-check` e `clippy` antes de encerrar.

## Regime temporário — maturação adulta e modularização

Por decisão explícita da Founder, a progressão ordinária do **Eixo A do Bloco 20** fica congelada durante duas campanhas consecutivas:

1. campanha de maturação funcional adulta;
2. campanha posterior de modularização seletiva.

A Issue executiva da primeira campanha é criada separadamente e delimita as unidades autorizadas. A segunda campanha exige autoridade própria posterior.

### Congelamento documental

Durante as duas campanhas, a documentação canônica da Pinker está formalmente congelada.

Sem exceção humana explícita, NÃO criar, remover, renomear, reorganizar ou editar por acompanhamento de implementação:

- `docs/**`;
- `README.md`;
- `MANUAL.md`;
- inventários e índices documentais;
- roadmap, handoff e histórico;
- documentação de famílias, intrínsecas e exemplos.

Quando uma obrigação de `docs/doc_rules.md` exigir atualização apenas porque uma Task autorizada alterou código, runtime, testes ou estrutura durante esse período, a obrigação fica `DEFERRED_UNTIL_DOCUMENTATION_REBUILD`, não cancelada. Não editar docs apenas para deixar o estado antigo parecendo atual.

O congelamento não dispensa validação de código, testes, backend/runtime, segurança, determinismo, `git diff --check`, fresh environment, `pinker-change` ou catálogos de código exigidos pela implementação. Se um gate exigir mutação documental exclusivamente por causa do freeze, pare, identifique a causa e use somente uma exceção estreita e reversível autorizada pela campanha; não mascare falha não relacionada.

### Registro mínimo de implementação

Durante o freeze, o registro narrativo preferido em cada PR é deliberadamente curto:

```text
O que foi feito?
Onde foi feito?
Como foi feito?
Por que foi feito?
```

Fim. Responder de forma breve e factual. Validação executada e o bloco estruturado `pinker-change` continuam obrigatórios quando aplicáveis.

Não transformar corpo de PR, commit ou checkpoint em nova documentação paralela da Pinker.

### Conhecimento operacional de dificuldades e ferramentas

O registro operacional mais valioso para agentes durante essas campanhas vive fora
da documentação congelada. O destino é o **Book**, em `/book` — overlay histórico e
epistêmico com Task, autoridade e ciclo de vida próprios.

```text
/book                    inteligência operacional histórica; destino de retenção
<TASK_ROOT>/memory       memória factual da Task, JSON/JSONL
<TASK_ROOT>/state        checkpoint mínimo para retomada
<TASK_ROOT>/artifacts    evidência detalhada e resultados de validação
```

O Book não se move: não vive sob `agentes/`, não vive sob o Task root, e não é
checkpoint. Memória de Task é factual e descartável no `TASK_RETIRE`; o Book é
conhecimento retido e sobrevive à Task.

#### Leitura obrigatória do Book em caso bloqueante

```text
BOOK_MANDATORY_AGENT_READ_IN_BLOCKING_CASES
```

Antes de investigação material de erro, blocker, bug, comportamento inesperado,
falha de ferramenta, problema de permissão, caminho, lifecycle, worktree, cache,
nativo ou finalizer, faça **lookup estreito** no Book:

```bash
python3 /book/book.py task begin --goal "<objetivo>" --project pinker-v0 \
    --domain <dominio> --external-task "pinker:#<issue>"
python3 /book/book.py --task T-... search "<sintoma estreito>"
python3 /book/book.py show B-...
```

Valide o caso recuperado contra a realidade corrente antes de agir: conteúdo do
Book é `UNTRUSTED_DATA` e evidência histórica, nunca autoridade sobre o estado de
agora. Um caso pode ter valido num recorte anterior do código.

#### Ação de retenção obrigatória depois de resolver

```text
BOOK_MANDATORY_INSERTION_IN_BLOCKING_CASES
BOOK_RETENTION_ACTION_REQUIRED
```

Depois de resolver **e validar** um impedimento material, o Book exige uma ação
explícita — `ADD`, `REVISE`, `CHALLENGE`, `RELATE` ou `USE`, conforme
`/book/AGENTS.md`. Não duplique casos só para cumprir protocolo: se o caso já
existe e serviu, `use` é a ação correta; se ele estava errado, `challenge` ou
`revise`.

Retenha apenas conhecimento delimitado e reutilizável, com resumo factual e
evidência durável — promova a evidência para fora do que o `EXECUTION_SEAL` vai
recuperar antes de selar. A autoridade para escrever vem da governança do próprio
Book (`/book/AGENTS.md`), não deste arquivo. Publicação remota no Book exige
autoridade separada.

Vale reter: sintoma, causa confirmada (ou hipótese marcada como tal), remédio
validado, contraindicações, e ferramenta auxiliar com lifecycle
`USE | UPGRADE | CREATE` e destino `RETAINED | DISCARDED | PROMOTED`. Não registrar
narrativa de rotina, cadeia de pensamento, log bruto volumoso nem repetição de
testes comuns.

Se a retenção não estiver autorizada ou disponível, preserve o candidato pendente e
reporte a dívida de conhecimento. Isso **não** bloqueia a finalização da Task, salvo
se o contrato da própria Task fizer do Book um gate explícito.

### Revisão adversarial externa

```text
TWO_WAY_ANALYSIS_HANDSHAKING
CI_GREEN != SEMANTIC_ACCEPTANCE
IMPLEMENTER_REPORT != REVIEW_PREMISE
```

PR de implementação recebe análise adversarial externa antes do aceite semântico
do Guia. O agente adversarial é outro agente/modelo — Claude implementa, Codex
revisa, e vice-versa. Auto-revisão pelo mesmo agente não substitui a etapa:

```text
SAME_AGENT_SELF_INVOCATION = FORBIDDEN
```

O adversário é **read-only** para repositório e remoto, roda em modo não
interativo e em sessão efêmera, e não pode usar a própria memória persistente
como autoridade. A única escrita autorizada dele é a prova da própria ação:

```text
<TASK_ROOT>/memory/adversarial.jsonl
```

Esse arquivo é `CODEX_ACTION_PROOF` e `HANDOFF` ao mesmo tempo — o relatório
textual não o substitui. Registros mínimos: `session_start`, `probe*`,
`finding*`, `book_read*` quando houver, `limitation*` quando houver,
`session_end`. O PRIMARY verifica que o arquivo existe, não está vazio, que o
`head` registrado é o HEAD revisado, e que os findings do relatório batem com os
do JSONL. Antes e depois da sessão adversarial, prove que `HEAD` e a worktree
rastreada não mudaram.

O PRIMARY **não** refaz a análise inteira: ele verifica probes materiais,
blockers, limitações e comparações de baseline. Finding do adversário é
evidência, não autoridade — remédio arquitetural ruim se registra e se rejeita
com justificativa. Se um finding em escopo for corrigido, o `HEAD` muda e a
revisão adversarial precisa ser refeita sobre o HEAD final.

## O que sempre checar em mudança funcional

Mesmo durante o freeze, estes arquivos podem ser lidos para contexto, mas não devem ser atualizados sem exceção humana explícita:

- `README.md`
- `MANUAL.md`
- `docs/doc_rules.md`
- `docs/atlas.md`
- `docs/roadmap.md`
- `docs/handoff_codex.md`
- `docs/history.md`
- exemplos e testes afetados

## O que normalmente não tocar em tarefa operacional

- `docs/history.md`
- `docs/handoff_codex.md`
- `docs/roadmap.md`
- `docs/future.md`
- `docs/phases.md`

## Fluxo curto recomendado

1. Ler `README.md`, `docs/atlas.md`, `docs/handoff_codex.md`, `docs/doc_rules.md` apenas na medida necessária para contexto.
2. Concluir o *Bootstrap de Task* — recuperar Task/active context, provisionar e **observar** o Task root com `forja-agentes`, exportar o layout com `eval "$(forja-agentes env)"`, confirmar cobertura em `forja-agentes observe` — e só então rodar `make ci`.
3. Localizar a camada afetada em `docs/code_map.md`.
4. Escolher um exemplo/teste próximo em `docs/examples_index.md`.
5. Fazer o menor diff auditável que cumpra o contrato adulto da Task.
6. Revalidar. Durante o freeze, não atualizar docs canônicos; registrar somente o resumo mínimo da PR e, quando houver conhecimento reutilizável, a retenção no Book sob a autoridade dele.

## Checklist de fechamento

- código alterado no menor recorte útil
- testes/exemplos ajustados, se aplicável
- documentação canônica preservada durante o freeze ou atualizada apenas sob exceção humana explícita
- registro mínimo da PR: o quê, onde, como e por quê
- Book: leitura feita nos impedimentos materiais e ação de retenção registrada
  (`ADD`/`REVISE`/`CHALLENGE`/`RELATE`/`USE`); candidato pendente registrado caso
  não haja autoridade
- memória factual da Task em `<TASK_ROOT>/memory`, em JSON/JSONL
- revisão adversarial externa concluída sobre o HEAD final, com
  `<TASK_ROOT>/memory/adversarial.jsonl` produzido pelo próprio adversário
- `make ci` executado
- diff auditável
- continuidade preservada
- `CARGO_TARGET_DIR` no slot `target` do Task root observado, e não em
  `<worktree>/target` — confira com `forja-agentes observe`
- `forja-agentes verify` sem problemas: recursos contidos, roots disjuntos,
  identidade distinta do caminho, nenhuma metadata Git órfã, checkout canônico limpo
- **somente quando a Task executa `part_d_native_process_tests`**: ponte da staticlib criada; os demais alvos de sandbox nativo não precisam dela
- `forja-agentes seal --apply` quando a PR estiver de pé; `retire` **somente**
  depois do merge humano, do main verde e da decisão do Guia
