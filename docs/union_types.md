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

<!-- @pinker-doc:end language.union-types.contract -->
