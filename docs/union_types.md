---
pinker-doc: 1
id: language.union-types
domain: language
kind: reference
status: active
parent: language
audience:
  - human
  - agent
related:
  - language.inline-assembly
  - engine
---

# Uniões estruturais tagged

<!-- @pinker-doc:start
id: language.union-types.contract
tags: [linguagem, uniao, tipos, tagged, encaixe]
aliases:
  - uniao estrutural
  - union types
summary: Define identidade canônica, injeção explícita, encaixe exaustivo e representação de uma palavra das uniões.
-->

Uma união é escrita `uniao<T1, T2, ...>`. Aliases são resolvidos, uniões
aninhadas são achatadas, duplicatas são removidas e membros são ordenados por
codificação canônica. Assim, `uniao<u8, verso>` e `uniao<verso, u8>` são o
mesmo tipo.

```pinker
nova valor: uniao<u8, verso> =
    (42 virar u8) virar uniao<verso, u8>;

encaixe valor {
    caso u8(x) { falar(x); }
    caso verso(x) { falar(x); }
}
```

O valor ocupa uma palavra e referencia descritor imutável com tipo internado,
tag e snapshot do payload. O lifetime é monotônico nesta fase.

## `encaixe` é um construto tipado

`encaixe` não é desdobrado no parser. A AST preserva o match de união como nó
próprio (`UnionMatchStmt`), com o scrutinee, o tipo **como escrito** de cada
braço, o binding, o corpo, a ordem de fonte e os spans. O parser não resolve
apelidos e não conhece tags.

A resolução acontece depois:

```text
tipo resolvido do membro
→ chave canônica compartilhada
→ UnionTypeIR internado
→ membro exato do registry
→ tag do registry
→ operação tipada de match/extração
```

A semântica resolve os apelidos integralmente, deriva a chave canônica de cada
braço pelo contrato compartilhado e só então valida a cobertura: cada braço deve
pertencer à união e todos os membros canônicos devem ser cobertos exatamente uma
vez. Dois apelidos do mesmo tipo canônico são o **mesmo** membro e são recusados
como duplicata:

```pinker
apelido byte_a = u8;
apelido byte_b = u8;

// recusado: `byte_a` e `byte_b` são o mesmo membro canônico
encaixe valor {
    caso byte_a(a) { falar(a); }
    caso byte_b(b) { falar(b); }
}
```

As tags pertencem ao registry canônico. O nome do apelido, a ordem dos braços e
a ordem textual da declaração da união não definem tag alguma:

```pinker
apelido aa = u8;
apelido zz = u64;

// `aa` é lexicalmente anterior, mas `u8` é canonicamente posterior a `u64`.
// O braço executado é o do tipo escrito, e a saída é 1007.
nova valor: uniao<aa, zz> = (7 virar aa) virar uniao<aa, zz>;
encaixe valor {
    caso aa(numero) { falar(1000 + (numero virar bombom)); }
    caso zz(numero) { falar(2000 + (numero virar bombom)); }
}
```

## Operações internas não são chamadas da linguagem

Ler a tag e abrir o payload são operações **internas tipadas** da IR
(`UnionTag` e `UnionExtract`), propagadas por CFG, seleção de instruções e
máquina abstrata. Elas não têm nome textual chamável, não passam pela resolução
comum de função, não aparecem como `Call` e não podem ser construídas pelo
parser. O símbolo de ABI do runtime é escolhido apenas no backend nativo.

O namespace `__pinker_internal_` permanece reservado ao compilador e recusado a
qualquer identificador vindo da fonte.

<!-- @pinker-doc:end language.union-types.contract -->
