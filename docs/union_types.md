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
summary: Define identidade canônica, injeção explícita, encaixe exaustivo, handle público de uma palavra e snapshot integral do payload — escalar, handle opaco ou agregado multi-palavra —, com a distinção explícita entre o que o pipeline representa, o que a sintaxe-fonte constrói e qual evidência executável cobre cada forma; e separa os dois domínios de armazenamento, a cota vitalícia de identidades públicas consumida só por `alocar` e o domínio interno de união com tetos, monotonicidade e diagnósticos próprios, idênticos no interpretador e no nativo.
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
| `MAX_UNION_BINDING_REGIONS` | 1 000 000 | teto de regiões de binding de extração no domínio interno |
| `MAX_UNION_BINDING_BYTES` | 256 MiB | teto agregado de bytes materializados para bindings |

Os snapshots vivem enquanto o processo vive; o crescimento é limitado por esses
tetos e não por coleta. Falha de alocação produz diagnóstico controlado, nunca
abort de alocador, e nenhum handle é publicado quando a alocação falha.

No runtime nativo, os três contadores — descritores, bytes de payload e bytes de
metadata — formam uma unidade de contabilidade própria. A reserva é uma função
pura sobre valores:

```text
reserve(orçamento_corrente, limites, tamanho_do_payload)
    -> orçamento_novo | motivo_da_recusa
```

O orçamento corrente entra por valor e o orçamento novo só existe no caminho de
sucesso: uma recusa não pode alterar contador nenhum, nem aquele que já havia
passado. O runtime de produção usa os limites canônicos da tabela acima; os
testes passam limites pequenos pelo mesmo parâmetro, o que permite exercitar
cada fronteira — último permitido, primeiro acima do teto e overflow de cada
contador — sem materializar um milhão de descritores. Não existe variável de
ambiente que altere a política, e debug e release se comportam igual.

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

### Contabilidade: identidade pública e domínio interno

Existem dois domínios de armazenamento, e eles não se misturam.

**Identidade pública** é a entrada de registro criada por `alocar`. Uma chamada
bem-sucedida de `alocar` consome exatamente uma, a cota é vitalícia por processo
(`liberar` devolve o armazenamento, não a identidade) e o esgotamento produz
`limite de identidades públicas esgotado`. O contrato completo está em
[MANUAL.md](../MANUAL.md#cota-vitalícia-de-identidades-públicas).

**Armazenamento interno de união** é tudo que a união materializa por conta
própria: o descritor, o snapshot imutável do payload e o storage do binding
produzido por uma extração agregada. Nada disso é uma identidade pública.

O que decorre disso, igualmente nos dois back-ends:

- construir uma união — escalar, handle opaco ou agregada — não consome
  identidade pública;
- extrair um payload agregado não consome identidade pública: no nativo o
  binding é um slot do frame (`leaq -offset(%rbp)`), no interpretador é uma
  região de uma arena interna própria, disjunta da arena pública;
- `liberar` recusa um endereço do domínio interno com
  `E-RUNTIME-MEM-FOREIGN-FREE`, e liberar memória pública não devolve nem altera
  o orçamento interno de uniões;
- apelidos, achatamento de união aninhada, `ResolvedTypeId`, travessia por
  chamada direta ou indireta e reinjeção de um binding extraído não mudam
  contagem alguma;
- construir uniões não altera a capacidade restante de `alocar`, e esgotar o
  domínio interno deixa a cota pública numericamente intacta.

Quais cotas são recuperáveis e quais domínios são monotônicos:

| domínio | unidade | recuperável | monotônico |
|---|---|---|---|
| identidade pública | entrada de registro de `alocar` | não (vitalícia) | sim |
| memória pública | bytes de uma região viva | sim, por `liberar` | não |
| descritores de união | descritor criado por injeção | não | sim |
| bytes de payload de união | bytes do snapshot | não | sim |
| metadata de descritores | cabeçalho por descritor | não | sim |
| binding de extração (interpretador) | região materializada por extração agregada | não | sim |

Os domínios de união permanecem monotônicos porque não existe contrato de
desalocação para uniões nesta fase. Isso não é uma promessa de reciclagem futura
nem um ownership novo: é a razão de cada um ter teto explícito.

Cada limite tem diagnóstico próprio, e nenhum deles reutiliza a mensagem do
limite público:

| diagnóstico | limite |
|---|---|
| `limite de identidades públicas esgotado` | cota vitalícia de `alocar` |
| `E-RUNTIME-UNION-DESCRIPTOR-BUDGET` | descritores internos |
| `E-RUNTIME-UNION-PAYLOAD-BUDGET` | bytes de payload internos |
| `E-RUNTIME-UNION-METADATA-BUDGET` | metadata de descritores |
| `E-RUNTIME-UNION-BINDING-BUDGET` | regiões de binding de extração |
| `E-RUNTIME-UNION-BINDING-BYTES` | bytes de binding de extração |
| `E-RUNTIME-UNION-BINDING-OVERFLOW` | overflow de layout no domínio interno |
| `E-RUNTIME-UNION-BINDING-METADATA` | metadata interna inconsistente |
| `E-RUNTIME-UNION-ALIGN` | alinhamento de payload acima do suportado |

Os endereços do domínio interno não são expostos como regiões públicas: o
registro de identidades não os contém, `liberar` não os aceita e o registro
interno vive fora do mapa endereçável, do mesmo modo que o registro público —
casts de inteiro para ponteiro não observam metadata de nenhum dos dois.

Uma diferença de realização permanece, e é deliberada: no nativo o storage do
binding é um slot do frame, reaproveitado a cada passagem pelo mesmo ponto de
extração; no interpretador não há frame de máquina, e a arena interna cresce
monotonicamente até `MAX_UNION_BINDING_BYTES`. Os dois têm limite explícito e
diagnóstico próprio; o teto do interpretador é o mais estrito dos dois.

### Capacidade do pipeline, superfície-fonte e evidência

A tabela de categorias descreve o que o **pipeline representa**. Isso não é o
mesmo que o conjunto de valores que a **sintaxe-fonte atual constrói**, e nenhum
dos dois substitui a evidência executável. As três colunas são distintas e estão
registradas aqui separadamente.

| forma | pipeline representa | fonte constrói hoje | evidência executável |
|---|---|---|---|
| payload escalar | sim | sim | `examples/fase248_unioes_estruturais_valido.pink` |
| payload handle opaco | sim | sim | `examples/fase248_unioes_estruturais_valido.pink` |
| array fixo `[bombom; N]` | sim | sim | `examples/hr3_uniao_agregado_imutavel_valido.pink`, `examples/hr3_uniao_extracoes_independentes_valido.pink` |
| `ninho` como payload | sim | sim | `examples/hr3_uniao_agregados_nominais_valido.pink` |
| `ninho`s nominais homorrepresentados | sim | sim | `examples/hr3_uniao_agregados_nominais_valido.pink` |
| reinjeção do payload extraído | sim | sim | `examples/hr3_uniao_agregados_nominais_valido.pink` |
| mutação do binding extraído | sim | sim | `examples/hr3_uniao_binding_extraido_mutavel_valido.pink` |
| agregado contendo agregado (`ninho` com array) | sim | parcialmente | `examples/hr3_uniao_agregado_aninhado_valido.pink` e programa IR sintético em `tests/pr411_hr3_terminal_evidence_tests.rs` |
| array fixo aninhado (`[[T; N]; M]`) | não | não | recusado pela semântica |

O caso parcial tem uma formulação exata. O `ninho` com campo de array fixo é
injetado, copiado e extraído integralmente pela fonte, e o exemplo versionado
observa a cabeça e a cauda do agregado — as duas fronteiras que só coincidem se
o array interno tiver sido copiado inteiro. O que a fonte ainda **não** oferece
é nome de campo para as células internas: não há construtor de array literal nem
acesso encadeado a campo agregado.

> O pipeline de payload suporta essa representação e é validado por programas
> IR/máquina executáveis. A sintaxe-fonte atual ainda não oferece um construtor
> direto para esse valor.

O programa IR sintético usado nessa prova não é um atalho de classificação: a IR
vem do lowering real, recebe uma cirurgia mínima nos deslocamentos internos e
atravessa `ir_validate`, `cfg_ir`, `cfg_ir_validate`, `instr_select`,
`instr_select_validate`, `abstract_machine` e `abstract_machine_validate` antes
de executar no interpretador e no binário ELF nativo, célula a célula.

Um binding de braço com payload `ninho` **é** o endereço do seu storage próprio
e aceita acesso de campo direto (`ninhado.cabeca`). Fora desse binding, o acesso
de campo continua exigindo a forma `(*ptr).campo`; passar um `ninho` por valor e
acessar seus campos permanece fora do recorte.

### Estado da revisão humana

Os cinco findings da revisão humana original da PR #411 — HR1, HR2, HR3, HR4 e
HR5 — estão corrigidos. A revisão foi concluída e a PR #411 foi mergeada na
`main`; nada aqui permanece em correção.

<!-- @pinker-doc:end language.union-types.contract -->
