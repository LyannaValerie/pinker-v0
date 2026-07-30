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

## Representação operacional e identidade semântica são coisas distintas

`TypeIR` é a **categoria operacional** de um valor: como ele é carregado,
armazenado e passado. Ela não identifica o tipo. Dois `ninho` diferentes
compartilham `TypeIR::Struct`; dois `leque` diferentes compartilham a
representação escalar; `carinho(u8) -> u8` e `carinho(u64) -> u64` compartilham
`TypeIR::Function`; `seta<u8>` e `seta<u64>` compartilham `TypeIR::Pointer`.

`ResolvedTypeId` é a **identidade semântica resolvida**: a identidade completa
do tipo, internada uma única vez por programa na tabela `ProgramIR.resolved_types`
e derivada da chave canônica de `union_canon`.

```text
tipo AST
→ resolução integral de apelidos
→ identidade semântica canônica
→ ResolvedTypeId internado
→ binding/valor/assinatura transporta ResolvedTypeId
→ membro da união possui o mesmo ResolvedTypeId
→ injeção exige igualdade exata
→ tag é copiada do membro exato
```

Os apelidos são transparentes **em profundidade**: `apelido A1 = Cor` e
`apelido A2 = A1` convergem para o `ResolvedTypeId` de `Cor`, e o mesmo vale
dentro de tipos compostos (`seta<A2>`, `carinho(A2) -> A2`, `[A2; 4]`). O texto
do apelido, o span, o nome do slot, a ordem de declaração e a ordem de iteração
de qualquer mapa nunca participam da identidade.

A identidade tem forma própria para cada categoria:

| categoria | identidade |
|---|---|
| `ninho Alfa` | nominal: nome declarado |
| `leque Cor` | nominal: nome declarado, apesar da representação escalar |
| `seta<T>` | volatilidade mais a identidade do apontado |
| `carinho(P...) -> R` | assinatura completa: identidades dos parâmetros e do retorno |
| `[T; N]` | identidade do elemento mais o tamanho |
| `uniao<...>` | identidades dos membros, na ordem canônica |
| escalares, `verso`, listas e mapas monomórficos, `nulo` | a própria representação, que já é injetiva |

A identidade acompanha o valor por todas as superfícies legais: declarações
locais, atribuições, parâmetros, retornos, chamadas diretas e indiretas,
ternários, valores callable, closures, capturas, extração de payload e
reinjeção.

## A injeção escolhe o membro por igualdade exata

A injeção `virar uniao<...>` localiza o membro comparando o `ResolvedTypeId` do
valor de origem com o `ResolvedTypeId` de cada membro do registry. Não existe
desempate por primeira ocorrência nem seleção por `TypeIR`:

```pinker
leque Cor { Rosa, Azul }
leque Tom { Claro, Escuro }

// `Tom` e `Cor` têm a mesma representação escalar e identidades diferentes.
// O braço executado é o de `Tom`.
nova valor: uniao<Cor, Tom> = Tom.Escuro virar uniao<Cor, Tom>;
encaixe valor {
    caso Cor(c) { falar(1000); }
    caso Tom(t) { falar(2000); }
}
```

A tag é **copiada** do membro exato no lowering. Nenhuma camada posterior
reescolhe membro: CFG, seleção de instruções, máquina abstrata, backend e
interpretador apenas transportam a identidade e a **verificam** contra o
registry.

Quando a identidade não pode ser determinada, o compilador falha em vez de
aproximar:

| diagnóstico | condição |
|---|---|
| `E-IR-TYPE-IDENTITY-LOST` | a identidade semântica se perdeu antes do ponto que a exige (apelido não resolvido, representação ambígua, ramos de ternário discordantes) |
| `E-IR-UNION-IDENTITY-DUPLICATE` | duas tags da mesma união carregam a mesma identidade resolvida |
| `E-IR-UNION-MEMBER-IDENTITY-MISMATCH` | tag e identidade descrevem membros diferentes, ou existe membro com a mesma representação e nenhum com a identidade exigida |

## Operações internas não são chamadas da linguagem

Ler a tag e abrir o payload são operações **internas tipadas** da IR
(`UnionTag` e `UnionExtract`), propagadas por CFG, seleção de instruções e
máquina abstrata. Elas não têm nome textual chamável, não passam pela resolução
comum de função, não aparecem como `Call` e não podem ser construídas pelo
parser. O símbolo de ABI do runtime é escolhido apenas no backend nativo.

O namespace `__pinker_internal_` permanece reservado ao compilador e recusado a
qualquer identificador vindo da fonte.

<!-- @pinker-doc:end language.union-types.contract -->
