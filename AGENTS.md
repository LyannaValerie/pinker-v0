# AGENTS.md

Guia operacional curto para agentes neste repositório. Não substitui `README.md`, `MANUAL.md` ou docs canônicos de `docs/`. Esta é a **fonte operacional canônica** para agentes; não crie um segundo contrato (`CLAUDE.md` só deve existir se uma integração o exigir fisicamente).

## Bootstrap de Task (antes de qualquer Cargo)

Em Task da Forja, execute este bootstrap **antes do primeiro `cargo`, `make` ou
`pink` da árvore**. Nenhum comando normativo deste arquivo deve rodar Cargo antes
dele.

**Fase 0 — sem build.** Recupere identidade e ambiente; ainda não compile.

1. Recupere a Task e o active context. O `TASK_ID` vem do observador canônico, não
   do prompt nem de um nome inventado:

   ```bash
   sudo -n forja-lifecycle show
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

3. Recupere os caminhos físicos da autoridade atual — **não os monte por
   concatenação a partir do `TASK_ID`**. O layout físico pode mudar; a identidade
   da Task, não.

   ```bash
   sudo -n forja-task-storage show          # namespaces root-gated provisionados
   fl --report --json                       # slots efêmeros registrados da Task
   git rev-parse --show-toplevel            # raiz real da worktree
   ```

   O caminho listado por `fl --report` sob `kind: cache` é, por definição, o slot
   de build que o finalizador sabe liberar. Use aquele valor; não o reconstrua.

   ```bash
   TASK_ID=$(sudo -n forja-lifecycle show | python3 -c 'import json,sys;print(json.load(sys.stdin)["context"]["task"])')
   TASKDIR=$(git rev-parse --show-toplevel)
   CARGO_TARGET_DIR=$(fl --report --json | python3 -c 'import json,sys;print(next(r["path"] for r in json.load(sys.stdin)["storage"]["resources"] if r["kind"]=="cache"))')
   export TASK_ID TASKDIR CARGO_TARGET_DIR
   mkdir -p "$CARGO_TARGET_DIR"
   ```

   `/pinker/worktrees/tasks/<task-id>` e `/pinker/caches/target/tasks/<task-id>`
   são o layout canônico *ilustrativo*; trate-os como verdade apenas quando o
   observador atual os confirmar.

4. Exporte `TMPDIR` somente para um namespace real e autorizado. Se o slot `tmp`
   da Task não existir ou não for gravável pelo perfil corrente, **não** aponte
   `TMPDIR` para ele: use o scratch autorizado do agente ou deixe o padrão, e
   registre a lacuna.

5. Confirme a cobertura antes de compilar. O `fl` precisa enxergar o seu diretório
   de build:

   ```bash
   fl --report --json
   ```

   Se `fl --report` não listar o seu `CARGO_TARGET_DIR`, ele não será liberado no
   fechamento. Isso é lacuna de cobertura a resolver **antes** de construir.

**Fase 1 — com build.** Só agora construa e use o `pink` da árvore para tudo que
dependa do estado corrente do repositório: `doc`, `nav`, `impacto`, `verificar`,
sincronização e manutenção de projeção.

### Por que o build precisa sair de `<worktree>/target`

Por padrão o Cargo grava em `<worktree>/target`, e `ci_env.sh` não define
`CARGO_TARGET_DIR`. Esse caminho não pertence a nenhum escopo efêmero registrado:
o finalizador `fl` classifica a worktree inteira como `preserved` e nunca a libera.
Medido em `issue-514` e `issue-520`: ambas encerraram com o finalizador reportando
`CLEAN_NOOP` e `reclaimed_bytes: 0` enquanto 7,3 GB e 7,0 GB de build ficavam para
trás. Com o alvo no slot registrado, o mesmo finalizador recuperou 8,06 GB.

### Como ler `CLEAN_NOOP` no fechamento

`CLEAN_NOOP` é resultado terminal legítimo do finalizador: significa que nada
registrado restava para liberar — inclusive quando os recursos já foram finalizados
ou reclamados numa passagem anterior autorizada. Não trate ausência de bytes como
prova de vazamento, e não fabrique trabalho para transformá-lo em `CLEAN`.

A suspeita só se justifica quando todas as condições valem ao mesmo tempo:

```text
primeira finalização após um build conhecido desta Task
+ nenhuma limpeza/finalização autorizada anterior
+ slot de cache registrado e esperado
+ bytes_before = 0
=> SUSPECT_UNCOVERED_BUILD, investigar antes de encerrar
```

E preserve explicitamente o caso oposto:

```text
já finalizado/reclamado legitimamente antes
+ CLEAN_NOOP
=> resultado limpo e válido
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

Herdam esse comportamento, via helper compartilhado:

```text
tests/native_cleanup_tests.rs
tests/native_process_control_tests.rs
tests/native_quarantine_recovery_tests.rs
tests/part_d_native_process_tests.rs
scripts/pinker-cleanup.sh
scripts/pinker-flake-runner.sh
```

Esses alvos compartilham a mesma raiz de sandbox. Ao investigar falha intermitente
neles sob execução paralela, considere interferência no sandbox compartilhado antes
de concluir regressão.

**2. A ponte da staticlib é exigida por um único alvo.**

| alvo | path exigido | precisa de ponte |
|---|---|---|
| `tests/part_d_native_process_tests.rs:114` | `<repo>/target/debug/libpinker_rt.a` | **sim** |
| demais alvos da lista acima | `<repo>/target/pinker-exec/` | não |

Só `part_d_native_process_tests` procura a staticlib pela raiz do repositório. Se a
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

Em ambos os casos `<worktree>/target` guarda apenas o symlink e o sandbox: dezenas
de kilobytes, não gigabytes. O peso do build continua no slot registrado.

### Como ler `CLEAN_NOOP` no fechamento

`CLEAN_NOOP` é resultado terminal legítimo do finalizador: significa que nada
registrado restava para liberar — inclusive quando os recursos já foram finalizados
ou reclamados numa passagem anterior autorizada. Não trate ausência de bytes como
prova de vazamento, e não fabrique trabalho para transformá-lo em `CLEAN`.

A suspeita só se justifica quando todas as condições valem ao mesmo tempo:

```text
primeira finalização após um build conhecido desta Task
+ nenhuma limpeza/finalização autorizada anterior
+ slot de cache registrado e esperado
+ bytes_before = 0
=> SUSPECT_UNCOVERED_BUILD, investigar antes de encerrar
```

E preserve explicitamente o caso oposto:

```text
já finalizado/reclamado legitimamente antes
+ CLEAN_NOOP
=> resultado limpo e válido
```

### Ponte da staticlib — condicional aos fixtures que usam paths históricos

Alguns fixtures nativos resolvem paths a partir da raiz do repositório e por isso
**não** enxergam `CARGO_TARGET_DIR`. Os casos comprovados hoje são:

| fixture | path histórico exigido |
|---|---|
| `tests/part_d_native_process_tests.rs:114` | `<repo>/target/debug/libpinker_rt.a` |
| sandbox de execução nativa | `<repo>/target/pinker-exec/` |

Isso é uma propriedade desses fixtures, **não** de todo gate nativo. Se a sua Task
não executa esse recorte, não crie ponte alguma.

Quando um desses gates for aplicável, ligue o artefato real ao caminho esperado
depois do primeiro build:

```bash
mkdir -p "$TASKDIR/target/debug"
ln -sfn "$CARGO_TARGET_DIR/debug/libpinker_rt.a" "$TASKDIR/target/debug/libpinker_rt.a"
```

Sem a ponte, os 8 testes de `part_d_native_process_tests` falham com
`staticlib nativa ausente` — falha de fixture, não regressão de código. Confira o
local do panic antes de investigar semântica. Com a ponte, `<worktree>/target`
guarda só o symlink e o sandbox: dezenas de kilobytes, não gigabytes.

### Caminho físico e `SUN_LEN`

Fixtures nativos bindam socket unix sob `<repo>/target/pinker-exec/`, e o limite de
`SUN_LEN` é 108 bytes. Uma worktree de caminho longo estoura esse limite dentro do
fixture, antes de qualquer código Pinker executar.

Encurte o **caminho físico** da worktree por mecanismo autorizado da Forja,
preservando o `TASK_ID`. Se não houver mecanismo autorizado disponível, classifique
como blocker de ambiente e reporte. Nunca altere ou encurte a identidade da Task
para caber num socket: identidade não é parâmetro de conveniência.

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
Subcomando (`nav`, `doc`, `doctor`, `verificar`, `estado`, `agente`) nunca leva o `--`
extra.

Marco documental e política forward-only: `.pinker/doc.toml`. Manifestos versionados: `.pinker/changes/`.

## Comandos padrão

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

### Memória operacional de dificuldades e ferramentas

O registro operacional mais valioso para agentes durante essas campanhas vive fora
da documentação congelada. O destino atual é o **Book**, o overlay histórico e
epistêmico com Task, autoridade e ciclo de vida próprios.

```text
Book                = inteligência operacional histórica; destino de nova retenção
/pinker/msg         = fonte LEGACY de migração, somente leitura
checkpoint          = estado operacional mínimo para retomada
artifacts/tasks/... = evidência detalhada e resultados de validação
```

Não escreva memória nova em `/pinker/msg`. Quando o arquivo existir, trate-o como
entrada histórica a migrar oportunamente, preservando a proveniência; não faça
dual-write nem backfill em massa. Se uma autoridade viva e explícita ainda exigir
escrita em `/pinker/msg` para uma operação específica, essa autoridade estreita
prevalece para aquela operação e o conflito deve ser registrado, não silenciado.

Retenha no Book apenas conhecimento delimitado e reutilizável, com resumo factual e
evidência durável — promova a evidência para fora de scratch antes da finalização
destrutiva. A autoridade para escrever vem da governança do próprio Book
(`/book/AGENTS.md` quando presente), não deste arquivo e não da capacidade de
acessar o repositório. Publicação remota no Book exige autoridade separada.

Vale reter: sintoma, causa confirmada (ou hipótese marcada como tal), remédio
validado, contraindicações, e ferramenta auxiliar com lifecycle
`USE | UPGRADE | CREATE` e destino `RETAINED | DISCARDED | PROMOTED`. Não registrar
narrativa de rotina, cadeia de pensamento, log bruto volumoso nem repetição de
testes comuns.

Se a retenção não estiver autorizada ou disponível, preserve o candidato pendente e
reporte a dívida de conhecimento. Isso **não** bloqueia a finalização da Task, salvo
se o contrato da própria Task fizer do Book um gate explícito.

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
2. Concluir o *Bootstrap de Task* — recuperar Task/active context, resolver os paths reais, configurar build/cache/`TMPDIR`, confirmar cobertura no `fl --report` — e só então rodar `make ci`.
3. Localizar a camada afetada em `docs/code_map.md`.
4. Escolher um exemplo/teste próximo em `docs/examples_index.md`.
5. Fazer o menor diff auditável que cumpra o contrato adulto da Task.
6. Revalidar. Durante o freeze, não atualizar docs canônicos; registrar somente o resumo mínimo da PR e, quando houver conhecimento reutilizável, a retenção no Book sob a autoridade dele.

## Checklist de fechamento

- código alterado no menor recorte útil
- testes/exemplos ajustados, se aplicável
- documentação canônica preservada durante o freeze ou atualizada apenas sob exceção humana explícita
- registro mínimo da PR: o quê, onde, como e por quê
- dificuldades e ferramentas auxiliares avaliadas para retenção no Book, quando houver valor reutilizável e autoridade para escrever; candidato pendente registrado caso contrário
- `make ci` executado
- diff auditável
- continuidade preservada
- build da Task gravado em `/pinker/caches/target/tasks/<task-id>`, e não em `<worktree>/target`
- **somente quando a Task executa `part_d_native_process_tests`**: ponte da staticlib criada; os demais alvos de sandbox nativo não precisam dela
- `fl --report` conferido antes de `fl`; `CLEAN_NOOP` interpretado conforme *Como ler `CLEAN_NOOP` no fechamento*, não como prova automática de vazamento
