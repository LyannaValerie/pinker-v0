# AGENTS.md

Guia operacional curto para agentes neste repositório. Não substitui `README.md`, `MANUAL.md` ou docs canônicos de `docs/`. Esta é a **fonte operacional canônica** para agentes; não crie um segundo contrato (`CLAUDE.md` só deve existir se uma integração o exigir fisicamente).

## Entrada pela Trama Pinker

A documentação é dual: portais Markdown para humanos, catálogos consultáveis para agentes. Para não varrer `docs/` ou `src/` indiscriminadamente:

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

### Diretório de build por Task (obrigatório em Task da Forja)

Por padrão o Cargo grava em `<worktree>/target`. Esse caminho **não pertence a
nenhum escopo efêmero registrado**: o finalizador `fl` classifica a worktree
inteira como `preserved` e nunca a libera. O resultado observado é um `fl` que
reporta `CLEAN_NOOP` com `reclaimed_bytes: 0` enquanto dezenas de gigabytes de
build ficam para trás.

Antes do primeiro `cargo`/`make` da Task, exporte o alvo para o slot que o `fl`
inventaria:

```bash
export TASK_ID=<task-id>                       # o mesmo id do active context
export TASKDIR=/pinker/worktrees/tasks/$TASK_ID
export CARGO_TARGET_DIR=/pinker/caches/target/tasks/$TASK_ID
export TMPDIR=/pinker/work/tasks/$TASK_ID/tmp  # quando o namespace existir
mkdir -p "$CARGO_TARGET_DIR"
cd "$TASKDIR"
```

Confirme que o slot entrou no escopo antes de compilar — o `fl` deve listar o
`kind: cache` apontando para o seu `CARGO_TARGET_DIR`:

```bash
fl --report --json
```

#### Ponte obrigatória para os gates nativos

Os gates nativos **não respeitam `CARGO_TARGET_DIR`**: `part_d_native_process_tests`
procura a staticlib literalmente em `<repo>/target/debug/libpinker_rt.a`
(`tests/part_d_native_process_tests.rs:114`), e o sandbox de execução é criado em
`<repo>/target/pinker-exec/`. Com o alvo fora da árvore, os 8 testes de
`part_d_native_process_tests` falham com `staticlib nativa ausente` — falha de
fixture, **não** regressão de código.

Ligue o artefato real ao caminho que o harness espera, depois do primeiro build:

```bash
mkdir -p "$TASKDIR/target/debug"
ln -sfn "$CARGO_TARGET_DIR/debug/libpinker_rt.a" "$TASKDIR/target/debug/libpinker_rt.a"
```

`<worktree>/target` continua existindo, mas passa a conter só o symlink e o sandbox
`pinker-exec` — dezenas de megabytes em vez de gigabytes. O `debug/` pesado, o
`doc/` do rustdoc e as dependências ficam no slot que o `fl` limpa.

Cuidado adicional com o comprimento do caminho: o fixture de processos binda socket
unix sob `<repo>/target/pinker-exec/`, e o limite de `SUN_LEN` é 108 bytes. Uma
worktree de caminho longo estoura esse limite dentro do fixture, antes de qualquer
código Pinker rodar. Mantenha o id da Task curto.

Se `fl --report` não listar o seu diretório de build, ele não será limpo no
fechamento. Isso é uma lacuna de cobertura a resolver **antes** de construir, não
depois.

No fechamento, `CLEAN_NOOP` com `reclaimed_bytes: 0` só é um sucesso legítimo se a
Task realmente não compilou nada. **Se a Task compilou e o slot de cache reporta
`bytes_before: 0`, o build vazou para fora do escopo**: localize-o e registre a
lacuna antes de encerrar.

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
  responsável por exportá-lo para `/pinker/caches/target/tasks/<task-id>` antes do
  primeiro build, conforme *Diretório de build por Task*.
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

O registro operacional mais valioso para agentes durante essas campanhas vive fora da documentação congelada, em:

```text
/pinker/msg/campanhas/<campanha>/<task-id>.md
```

Para a primeira campanha use:

```text
/pinker/msg/campanhas/maturacao-adulta/<task-id>.md
```

O arquivo deve ser curto e conter apenas o que possuir valor de reutilização:

```markdown
# Dificuldades
- sintoma / bloqueio
- causa confirmada ou hipótese explicitamente marcada
- resolução, workaround ou decisão de parar
- lição reutilizável, quando houver

# Ferramentas auxiliares
- ferramenta usada, criada ou atualizada
- lifecycle: USE | UPGRADE | CREATE
- caminho
- finalidade
- destino: RETAINED | DISCARDED | PROMOTED
```

Se não houve dificuldade material ou ferramenta auxiliar, registrar `nenhuma` na seção correspondente. Não registrar narrativa de rotina, cadeia de pensamento, log bruto volumoso ou repetição de testes comuns.

Checkpoints e `/pinker/artifacts/tasks/<task-id>/` continuam responsáveis por estado de retomada, evidência detalhada e resultados de validação. `/pinker/msg` registra memória operacional humana/agente, não substitui checkpoint nem artifact.

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
2. Exportar `TASK_ID`, `TASKDIR` e `CARGO_TARGET_DIR` (ver *Diretório de build por Task*) e rodar `make ci`.
3. Localizar a camada afetada em `docs/code_map.md`.
4. Escolher um exemplo/teste próximo em `docs/examples_index.md`.
5. Fazer o menor diff auditável que cumpra o contrato adulto da Task.
6. Revalidar. Durante o freeze, não atualizar docs canônicos; registrar somente o resumo mínimo da PR e a memória operacional em `/pinker/msg`.

## Checklist de fechamento

- código alterado no menor recorte útil
- testes/exemplos ajustados, se aplicável
- documentação canônica preservada durante o freeze ou atualizada apenas sob exceção humana explícita
- registro mínimo da PR: o quê, onde, como e por quê
- dificuldades e ferramentas auxiliares registradas em `/pinker/msg`, quando aplicável
- `make ci` executado
- diff auditável
- continuidade preservada
- build da Task gravado em `/pinker/caches/target/tasks/<task-id>`, e não em `<worktree>/target`
- ponte `<worktree>/target/debug/libpinker_rt.a` criada, sem a qual os gates nativos falham por fixture
- `fl --report` conferido antes de `fl`: se a Task compilou, o slot `kind: cache` precisa acusar bytes; `CLEAN_NOOP` com `reclaimed_bytes: 0` após um build é vazamento, não limpeza
