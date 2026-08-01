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

A gramática de statement do GNU as, depois de removidos comentários e labels,
tem exatamente três formas: **diretiva** (começa com `.`), **atribuição de
símbolo** (`nome = expressão`) e **instrução**. As duas primeiras definem ou
alteram símbolo, e `sussurro` não define símbolo; as duas são rejeitadas por
construção, independentemente do nome usado — inclusive nomes que nenhuma lista
conteria. O que resta é a instrução, cujo texto de operandos é entregue ao
assembler real.

A recusa da atribuição vale com qualquer espaçamento (nenhum, largo ou com tab),
depois de `;`, de comentário já removido, de newline normalizado (inclusive
CRLF) e de label local numérico, e cobre a forma `==` do dialeto. Formas apenas
parecidas mantêm a classificação própria: `nome:` é label nominal e `= 1` sem
token à frente é token estrutural inesperado.

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
- atribuição de símbolo (`nome = expressão`, `nome == expressão`), inclusive na
  forma de alias para um símbolo existente;
- criação ou troca de seção; criação ou alteração de símbolos;
- macros e repetição do assembler; inclusão; troca de sintaxe; troca de modo de
  código; CFI; dados embutidos;
- um separador de statement dentro de região citada, que faria o scanner e o
  assembler discordarem sobre o fim do statement.

Diagnósticos: `E-SEMANTIC-ASM-DIRECTIVE`, `E-SEMANTIC-ASM-NAMED-LABEL`,
`E-SEMANTIC-ASM-SYMBOL-ASSIGN`, `E-SEMANTIC-ASM-UNEXPECTED-TOKEN`,
`E-SEMANTIC-ASM-UNTERMINATED-QUOTE`, `E-SEMANTIC-ASM-UNTERMINATED-COMMENT`,
`E-SEMANTIC-ASM-SEPARATOR-IN-QUOTE`, `E-SEMANTIC-ASM-NUL`.

### Limite conhecido

Não existe lista positiva de mnemônicos. O conjunto de instruções x86-64
aceitas depende da versão do assembler e das extensões habilitadas, não tem
autoridade única publicada e não poderia ser mantido completo — uma lista
parcial seria pior que nenhuma, porque pareceria integral. Por isso a política
estrutural governa a **forma** do statement, e o texto de operandos de uma
instrução é entregue ao assembler real, que é a autoridade sobre ele. O que a
fonte não pode provar é provado sobre o objeto produzido, na seção seguinte.

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
emitido. Diagnóstico do envelope: `E-BACKEND-ASM-ENVELOPE`.

## Invariante do artefato

`pink build --nativo` verifica o objeto realmente produzido antes de linkar —
não apenas em fixture de teste. A baseline é explícita e derivada do próprio
assembly emitido: o mesmo `.s` com os envelopes removidos por inteiro. As duas
variantes são montadas pelo mesmo driver C e comparadas, de modo que **todo**
delta é atribuível ao bloco; nada produzido pelo compilador ou pelo toolchain é
confundido com produção do autor.

A superfície comparada é o conjunto de seções e o conjunto de símbolos
**definidos** — nome, ligação, visibilidade, tipo, seção e tamanho. Símbolo
novo, alias novo, seção nova e mudança de ligação, de visibilidade ou de tamanho
em símbolo reservado do runtime aparecem como delta e abortam o build com
`E-BACKEND-ASM-ARTIFACT`. Símbolos apenas referenciados (`SHN_UNDEF`) ficam de
fora, porque referência a símbolo já existente é aceita pelo contrato.

O ELF é lido por um leitor próprio de ELF64 (`src/elf.rs`), e não pela saída
textual de `readelf` ou `nm`: o workspace não tem dependências externas, então
não havia parser de objeto a reutilizar, e saída textual de ferramenta muda
entre versões e locales. Sob `PINKER_EXIGE_NATIVO=1`, a ausência do driver C
bloqueia a evidência — não é pulada em silêncio.

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
