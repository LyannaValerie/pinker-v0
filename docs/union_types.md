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

## O payload é um snapshot, não uma palavra

O **valor público** de uma união continua sendo um handle de uma palavra: um
ponteiro para um descritor imutável. O que mudou é o descritor.

O descritor guarda um snapshot alinhado do payload **completo**. Não há mais
limite de uma palavra, o payload não é serializado dentro de um `u64` e o
descritor nunca referencia o storage do chamador.

```text
valor de origem
→ identidade semântica resolvida
→ membro exato do registry
→ layout classificado e validado
→ endereço do valor representacional
→ cópia integral para storage imutável do descritor
```

```text
encaixe
→ validar identidade da união
→ validar tag
→ validar layout esperado
→ copiar payload integral para storage novo do binding
→ executar o braço
```

### Três representações, decididas uma única vez

Todo membro é classificado por `crate::union_payload::classify_union_payload`,
a **única** autoridade de classificação do pipeline. Semântica, lowering,
registry, validadores de IR/CFG/seleção/máquina, interpretador, backend e a
validação da ABI do runtime consomem o mesmo resultado.

| categoria | membros | tamanho e alinhamento | cópia |
|---|---|---|---|
| escalar | `bombom`, `u8..u64`, `i8..i64`, `logica`, `leque` sem carga | largura real do tipo | por valor |
| handle opaco | `verso`, listas, mapas, `seta<T>`, ponteiros de função, callables e closures, objetos de trato, uniões já materializadas | uma palavra, por definição da categoria | rasa, por contrato |
| agregado | `ninho`, array fixo, apelidos resolvidos deles, agregados aninhados | layout estático completo, incluindo padding | integral, byte a byte |

Apelidos são transparentes em profundidade também aqui: dois apelidos do mesmo
agregado têm o mesmo `ResolvedTypeId`, a mesma classificação, o mesmo layout e a
mesma seleção de membro.

A cópia rasa dos handles é deliberada e vale inclusive **dentro** de um
agregado: um `verso` guardado num campo continua apontando para o mesmo
descritor de texto depois da injeção, exatamente como no runtime nativo. O que
é copiado profundamente é o storage do agregado.

### Não existe mais o fallback `(8, 8)`

Até HR3, qualquer erro de layout de um tipo não `nulo` era convertido em
`(8, 8)`. Isso deixava metadata falsa atravessar semântica, IR e validadores, e
a incoerência só aparecia — quando aparecia — na criação nativa do descritor.

Agora um tipo sem representação de payload conhecida é recusado **antes da IR
validada**, com código estável:

| código | motivo |
|---|---|
| `E-SEMANTIC-UNION-PAYLOAD-LAYOUT` | layout desconhecido, tipo inexistente, apelido ou ninho recursivo, overflow de layout |
| `E-SEMANTIC-UNION-PAYLOAD-SIZE` | tamanho zero ou acima do limite por payload |
| `E-SEMANTIC-UNION-PAYLOAD-ALIGN` | alinhamento zero, não potência de dois ou acima do suportado |
| `E-SEMANTIC-UNION-PAYLOAD-REPRESENTATION` | `nulo`, genérico não monomorfizado, tipo sem representação runtime definida |

Os validadores de IR, CFG, seleção e máquina repetem a defesa em vez de confiar
na origem.

### Limites explícitos

O repositório não possuía limite canônico aplicável a agregados de união. Os
valores abaixo são escolhidos explicitamente, são finitos, não dependem do
profile de compilação e são revalidados no runtime nativo e no interpretador com
operações checadas.

| constante | valor | papel |
|---|---|---|
| `MAX_UNION_PAYLOAD_BYTES` | 4096 | teto por payload; uma página |
| `MAX_UNION_PAYLOAD_ALIGN` | 16 | teto de alinhamento, coerente com o alinhamento de pilha da SysV |
| `MAX_UNION_DESCRIPTORS` | 1 000 000 | teto de descritores vivos, na ordem de grandeza de `MAX_IDENTIDADES_PUBLICAS` |
| `MAX_UNION_TOTAL_PAYLOAD_BYTES` | 256 MiB | teto agregado de bytes de snapshot |
| `MAX_UNION_METADATA_BYTES` | derivado | teto de metadata de descritores |

Os snapshots vivem enquanto o processo vive; o crescimento é limitado por esses
tetos e não por coleta. Falha de alocação produz diagnóstico controlado, nunca
abort de alocador.

### ABI interna

```text
pinker_uniao_criar(union_type_id, tag, payload_size, payload_align, payload_source_ptr) -> handle
pinker_uniao_tag(handle, expected_union_type_id) -> tag
pinker_uniao_copiar_payload(handle, expected_union_type_id, expected_tag, expected_size, expected_align, destination_ptr)
```

A criação recebe **endereço**, nunca o payload reempacotado numa palavra. O
backend materializa scratch alinhado do tamanho real para escalares e handles e
passa o endereço da representação completa para agregados. A extração copia para
storage novo do binding: o ponteiro interno do descritor nunca é devolvido, e
duas extrações da mesma união não compartilham memória.

O handle é validado antes de qualquer leitura — marca, identidade da união, tag,
tamanho e alinhamento — e um handle que não tenha sido criado por este runtime
nunca é dereferenciado.

### Independência observável

```text
origem criada → união injetada → origem modificada → encaixe observa o snapshot anterior
união extraída → binding modificado → novo encaixe → snapshot original permanece
duas extrações → storages distintos → valores inicialmente iguais
```

Esses contratos valem igualmente no interpretador e no caminho nativo, com
paridade de stdout, stderr, código de saída, braço selecionado e conteúdo
integral do payload.

### Estado da revisão humana

Os cinco findings da revisão humana original da PR #411 — HR1, HR2, HR3, HR4 e
HR5 — estão corrigidos. A PR continua exigindo nova revisão humana integral; não
há aprovação nem autorização de merge registradas aqui.

<!-- @pinker-doc:end language.union-types.contract -->
