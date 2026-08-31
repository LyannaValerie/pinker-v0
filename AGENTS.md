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
- Reconstruções históricas usam o contrato corrente de projeções. Regiões
  removidas podem ser declaradas como `materialize-region` no snapshot que
  possui o fato; os oito campos estáveis são suficientes.
- A ordem é `exclude -> override -> materialize`. Colisão de região corrente
  falha fechada.
- Medidas e projeções históricas `FROZEN` são imutáveis. Nunca recalibre
  `regions`, `length`, `fnv1a64` ou a projeção estável para esconder drift.
- Documentação só muda quando a Task exige ajuste de superfície; não faça
  rebuild documental amplo.

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
- Após CI e Trama remotos verdes, pare em `PR_GREEN_AWAITING_HUMAN_DECISION`.
