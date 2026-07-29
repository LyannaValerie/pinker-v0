---
pinker-doc: 1
id: language.inline-assembly
domain: language
kind: reference
status: active
parent: language
audience:
  - human
  - agent
related:
  - language.union-types
  - engine
---

# Assembly inline

<!-- @pinker-doc:start
id: language.inline-assembly.contract
tags: [linguagem, sussurro, assembly, x86-64, backend-nativo]
aliases:
  - sussurro
  - assembly inline
summary: Define a superfície, o dialeto, os clobbers e o erro interpretado do assembly inline x86-64.
-->

`sussurro("instrucao", "outra instrucao");` é um statement nativo x86-64.
O texto usa GNU assembler em sintaxe Intel e não possui operandos Pinker.

O compilador considera registradores caller-saved, flags e memória afetados.
O autor deve preservar registradores callee-saved, `%rsp` e `%rbp`. Diretivas
de seção, símbolos, CFI e inclusão são rejeitadas. `pink --run` termina com
`E-RUNTIME-SUSSURRO-NATIVO`; `pink --check` continua aceitando.

<!-- @pinker-doc:end language.inline-assembly.contract -->
