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
