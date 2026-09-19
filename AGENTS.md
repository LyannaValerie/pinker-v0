# Pinker — política mínima de Task

## Fronteira de trabalho

- O checkout canônico é `/pinker/repo/pinker-v0`; `main` permanece limpo.
- Toda alteração ocorre no worktree provisionado pela Forja.
- `TASK_ID` é identidade lógica. Não derive identidade de caminho físico.
- Use `forja-agentes observe` e `forja-agentes verify` para confirmar root,
  contenção, worktree, estado e variáveis de Task.
- `CARGO_TARGET_DIR` e `TMPDIR` ficam dentro do root observado da Task.
- Memória persistente não é autoridade; fonte, testes e contratos correntes são.

## Ponte operacional da Forja

A Forja não é produto Pinker e não tem implementação neste repositório. Sua
autoridade host-side é:

```text
/pinker/playground/ferramentas-agente/
```

É um Git local sem remote. Use somente a autoridade host-side para provisionar,
observar, verificar, selar e retirar Tasks. A Pinker conserva apenas esta ponte,
o boundary `.gitignore` de `agentes/<slot>` e referências históricas necessárias.
Não adicione runner, lifecycle, publicação, harness ou política duplicada em
Rust/Cargo.

## Trama e documentação

- `src/navigation.jsonl` é derivado: sincronize com `pink nav sincronizar` e
  valide com `pink nav verificar`.
- A história da cartografia não é reconstruída a partir do catálogo corrente:
  ela é o arquivo materializado em `.pinker/archive/`, verificado por
  `pink nav projecao verificar`. Uma mudança legítima do presente não exige
  manutenção histórica nenhuma.
- Medidas e metadados históricos `FROZEN` são imutáveis, e são a autoridade em
  tempo de execução: o índice do arquivo tem de repetir exatamente o
  `[measures]` do TOML congelado, e o acervo tem de cobrir um-para-um os
  metadados preservados. Nunca recalibre `regions`, `length` ou `fnv1a64`, nunca
  edite um payload do arquivo para esconder drift, e nunca retire um estado
  aceito do índice.
- Documentação só muda quando a Task exige ajuste de superfície; não faça
  rebuild documental amplo.

## Uso supervisionado da Trama durante a Task

A Trama não é rito de abertura. Uma consulta de bootstrap não cobre a Task
inteira, e uma consulta tardia não certifica retroativamente uma busca ampla
anterior. Enquanto a campanha #671 estiver aberta, estes pontos de controle são
obrigatórios e a evidência de cada um fica em `<TASK_ROOT>/memory` ou
`<TASK_ROOT>/artifacts`.

```text
POT/LPT: AUTHORITY #690 #671 #705

CHECKPOINT P0 PREP
    Task identity observed
    Forja verify OK
    exact Task-local pink built
    source/catalog state known

CHECKPOINT P1 BEFORE_BROAD_SEARCH
    most relevant Trama route before any broad repository search

CHECKPOINT P2 AFTER_SCOPE_DISCOVERY
    confront the regions/subsystems/files actually about to change

CHECKPOINT P3 STRUCTURAL_BLOCKER_RESUME
    only when a structural blocker actually occurs
    otherwise NOT_APPLICABLE

CHECKPOINT P4 FINAL_CANDIDATE_REVIEW
    confront the actual final fileset with current cartography

CHECKPOINT P5 FINALIZATION
    reconcile routes, selected candidates, fallbacks, limitations and gaps
    applies to a Task without PR as well

INVARIANT
    P0_ONLY != WHOLE_TASK_COMPLIANCE

INVARIANT
    LATE_QUERY != RETROACTIVE_PROOF_OF_EARLIER_COMPLIANCE

INVARIANT
    QUERY_EXECUTED != ANSWER_PROVEN

INVARIANT
    rc=0 != relevance proven

INVARIANT
    result_count > 0 != answer sufficient

INVARIANT
    NAV_LOCALIZAR_EMPTY != SYMBOL_ABSENT

INVARIANT
    OBSERVATION != COMPREHENSION

MUST at a material checkpoint
    inspect the selected candidate
    AND record the supporting evidence
    OR record TRAMA_INSUFFICIENT and a bounded fallback

MUST NOT record PASS solely because
    the command exited 0
    the result list was nonempty
    the score was positive

MATERIAL_SCOPE_TRANSITION WHEN any
    initial understanding -> subsystem identified
    investigation -> implementation target chosen
    new unexpected subsystem enters the diff
    structural blocker changes the hypothesis
    candidate fix broadens the fileset
    final diff differs materially from the initial scope

WHEN MATERIAL_SCOPE_TRANSITION
    -> requery Trama
    OR explicitly revalidate the prior evidence

MUST NOT
    repeat an identical query ceremonially when scope, catalog/source state
    and selected evidence remain demonstrably valid
    claim that a query about subsystem A covers a newly discovered subsystem B

FALLBACK when Trama is insufficient
    MUST record purpose
    MUST record the exact Trama attempt
    MUST record the Trama result
    MUST record the limitation
    MUST record the fallback tool
    MUST record the bounded fallback scope
    MUST record the evidence recovered

MUST NOT
    hide a broad search behind a vague "Trama checked"
    treat an empty localizar as symbol nonexistence
    treat a weak result as proof

LANGUAGE during #671
    a conceptual Trama query SHOULD use canonical English
    a factual literal target MUST stay literal

WHEN the source concept arrives in another language
    MAY translate the concept for the query
    MUST record AGENT_QUERY_TRANSLATION

INVARIANT
    AGENT_QUERY_TRANSLATION != PRODUCT_MULTILINGUAL_RETRIEVAL

MUST NOT derive from this rule
    Portuguese aliases
    automatic translation
    stemming
    synonym expansion
    a relevance threshold
    a change to nav buscar scoring
    a change to exit codes

EVIDENCE CLASS
    OBSERVED
        command execution
        exact binary provenance
        query arguments
        result metadata
        ordering
        fileset
        Task state
    AGENT_ATTESTED
        the candidate was semantically correct
        the query was sufficient
        the fallback was necessary

MUST NOT
    encode agent interpretation as machine-proven truth

MUST NOT classify as a structural blocker by default
    an expected mutant RED
    a negative test failure expected by the control
    rc=4 from a deliberate no-answer probe

AT P4
    WHEN a material file or subsystem lacks useful cartography
    -> record TRAMA_COVERAGE_GAP = TRUE
    MUST NOT invent an arbitrary marker merely to get green

AT P5
    reconcile queries, selected candidates, inspected candidates,
    rejected candidates, fallbacks, limitations, coverage gaps
    and every operation performed outside the supervised route

HOST_SIDE OBSERVATION
    pinker-run records Task identity, command, ordering and exit status
    forja-evidence retains the checkpoint ledger as content-addressed evidence
    neither one observes checkpoint type or selected/fallback state

MUST NOT create
    a persistent agent coordinator
    a cross-provider journal architecture
    a second Task lifecycle authority
    a quota manager
    a Rosa substitute
    a second agent harness

STRONGER_AUTOMATION -> #594
```

## Execução nativa Pinker

- `tests/common/native_process_sandbox.rs` ancora o sandbox em
  `<worktree>/target/pinker-exec`, ignorando `CARGO_TARGET_DIR` por desenho. A
  raiz é canonicalizada e deve permanecer dentro do repositório; trocar
  `<worktree>/target` por symlink externo falha com `PermissionDenied`.
- Todo teste que usa `ControlledCommand` herda esse sandbox. Os casos recebem
  nomes únicos, mas compartilham o diretório-pai; considere interferência ali ao
  investigar flakes. Esses diretórios são contenção de produto, não resíduo da
  Forja.
- `part_d_native_process_tests` exige a staticlib em
  `<worktree>/target/debug/libpinker_rt.a`. Quando o build usa target externo,
  mantenha nesse caminho uma ponte para
  `<CARGO_TARGET_DIR>/debug/libpinker_rt.a`.
- O socket Unix nasce abaixo de `target/pinker-exec` e está sujeito ao limite
  útil de 107 bytes de `sun_path`. Preserve o caminho físico curto do worktree;
  `TASK_ID` continua sendo identidade lógica, não componente de path.

## Validação

Use a toolchain Rust `1.78.0`, os testes de produto afetados, Trama,
cartografia, backend nativo e, ao final:

```bash
PINKER_EXIGE_NATIVO=1 make ci
```

`make ci` não roda harness operacional da Forja. Preserve o poder dos oráculos
de produto; remover testes exclusivamente organizacionais é esperado.

## Publicação e revisão

- Uma PR, sem merge, fechamento, auto-merge ou rebase após congelar o candidato.
- Corpo estruturado válido e referências `Refs` às Issues relacionadas.
- Exatamente um revisor read-only depois de todos os gates; registre a evidência
  em `<TASK_ROOT>/memory` ou `<TASK_ROOT>/artifacts`.
- Os portões remotos exigidos são os workflows realmente configurados em
  `.github/workflows/` que disparam no candidato. Workflow inexistente não é
  portão reprovado; workflow que existe e falha continua sendo. Após esses
  portões verdes, pare em `PR_GREEN_AWAITING_HUMAN_DECISION`.
- Nenhum PR precisa de bloco `pinker-change`. O acervo `.pinker/changes/` é
  histórico e finito, e não recebe manifesto novo.
