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
O autor deve preservar registradores callee-saved, `%rsp` e `%rbp`.
`pink --run` termina com `E-RUNTIME-SUSSURRO-NATIVO`; `pink --check` continua
aceitando. A interpretação permanece não suportada.

## Política estrutural

A validação é feita por um scanner estrutural determinístico de *statements*,
não por uma lista de diretivas proibidas. O scanner normaliza continuações
`\`+newline, divide statements por newline e por `;` fora de comentários e de
regiões citadas, remove comentários de linha `#` e de bloco `/* */` sem
interpretar o conteúdo, e recusa citação ou comentário não terminado.

Depois da remoção de labels e comentários, toda diretiva do assembler começa um
statement com `.`. Por isso **todas** as diretivas são rejeitadas por
construção, independentemente do nome — inclusive as que nenhuma lista conteria.

Aceito:

- instruções GNU assembler x86-64 em sintaxe Intel;
- múltiplas instruções, inclusive separadas por `;`;
- comentários de linha e de bloco;
- labels locais numéricos (`1:`) e referências `Nf`/`Nb`;
- referências a símbolos já existentes;
- segment overrides em operandos (`fs:[0]`), que não são labels.

Rejeitado:

- qualquer diretiva assembler escrita pelo autor;
- definição de label nominal (`nome:`, `.Lnome:`);
- criação ou troca de seção; criação ou alteração de símbolos;
- macros e repetição do assembler; inclusão; troca de sintaxe; troca de modo de
  código; CFI; dados embutidos;
- um separador de statement dentro de região citada, que faria o scanner e o
  assembler discordarem sobre o fim do statement.

Diagnósticos: `E-SEMANTIC-ASM-DIRECTIVE`, `E-SEMANTIC-ASM-NAMED-LABEL`,
`E-SEMANTIC-ASM-UNEXPECTED-TOKEN`, `E-SEMANTIC-ASM-UNTERMINATED-QUOTE`,
`E-SEMANTIC-ASM-UNTERMINATED-COMMENT`, `E-SEMANTIC-ASM-SEPARATOR-IN-QUOTE`,
`E-SEMANTIC-ASM-NUL`.

## Envelope do backend

Cada bloco é delimitado por sentinelas geradas pelo compilador — nenhuma vem da
fonte:

```
# PINKER-SUSSURRO-BEGIN:<id>
.intel_syntax noprefix
...
.att_syntax prefix
# PINKER-SUSSURRO-END:<id>
```

A validação do envelope confirma que cada begin tem exatamente um end, que os
identificadores são únicos, que os wrappers de sintaxe estão balanceados e que a
sintaxe AT&T é restaurada, e reaplica a política estrutural ao texto realmente
emitido. Depois da geração, o objeto montado é inspecionado com `readelf`: o
bloco não pode criar seção nem símbolo nomeado adicional. Sob
`PINKER_EXIGE_NATIVO=1`, a ausência das ferramentas de inspeção bloqueia — não é
pulada em silêncio. Diagnóstico do envelope: `E-BACKEND-ASM-ENVELOPE`.

## Namespace interno

O prefixo `__pinker_internal_` é reservado ao compilador e recusado em toda
posição de identificador originada da fonte — declaração de função, variável,
parâmetro, constante, apelido, ninho, leque, trato, método e campo, e também
qualquer referência ou chamada. O diagnóstico é
`E-SEMANTIC-RESERVED-NAMESPACE`.

A fronteira fica no ponto em que o texto da fonte se torna um identificador, de
modo que nenhum consumidor a jusante pode observar um identificador reservado.
Identificadores sintéticos construídos diretamente pelo compilador não são
lexados e portanto não passam por essa fronteira: os serviços internos de
runtime e de backend continuam disponíveis ao compilador.

<!-- @pinker-doc:end language.inline-assembly.contract -->
