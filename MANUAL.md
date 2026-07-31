# Manual da linguagem Pinker (estado atual)

Este manual apresenta a **Pinker como ela existe hoje**, no recorte já implementado no projeto.

## 1) O que é a Pinker

A Pinker é uma linguagem com sintaxe em português e vocabulário próprio, com termos como `carinho`, `mimo`, `talvez` e `verso`.
Aqui o foco é prático: mostrar como escrever programas que funcionam no estado atual do frontend e do runtime, sem antecipar recursos ainda não entregues.

## 2) Como ler a Pinker

A Pinker usa palavras-chave próprias, então o código pode soar diferente à primeira vista.
Use este manual como referência de **uso atual e canônico**: cada seção descreve apenas construções e recortes que já funcionam hoje.

## 3) Estrutura básica de um programa

Um programa típico tem:
- `pacote ...;` no topo;
- função de entrada `principal`;
- funções com `carinho`;
- retorno com `mimo`;
- variáveis com `nova` e mutabilidade com `muda`.

```pink
pacote main;

carinho somar(a: bombom, b: bombom) -> bombom {
    mimo a + b;
}

carinho principal() -> bombom {
    nova muda total: bombom = 0;
    total = somar(40, 2);
    mimo total;
}
```

## 4) Tipos básicos e valores

### `bombom`
Inteiro base da linguagem (u64 no subset atual):

```pink
nova n: bombom = 42;
```

### `logica`
Booleano com `verdade` e `falso`:

```pink
nova ativo: logica = verdade;
```

### `verso`
Texto (string) com literal entre aspas:

```pink
nova nome: verso = "Pinker";
```

Também existem tipos inteiros fixos (`u8..u64`, `i8..i64`) no estado atual.

Coleções: `lista<bombom>`, `lista<verso>` e, desde a Fase 211, `lista<T>` com `T` sendo qualquer `leque` declarado — todas operadas pelas intrínsecas genéricas `lista_criar` (exige anotação em `nova`), `lista_anexar`, `lista_obter`, `lista_tamanho`, `lista_definir`, `lista_tirar_ultimo` e `lista_inserir`. Desde a Fase 233, as quatro combinações públicas de `mapa<K,V>` (`verso`/`bombom`) também têm fachada genérica: `mapa_criar`, `mapa_definir`, `mapa_obter`, `mapa_tem`, `mapa_tamanho` e `mapa_remover`. Desde a Fase 235, essas operações aceitam o mapa como expressão tipada no primeiro argumento, não apenas variável local/parâmetro reconhecido pelo parser.

```pink
nova tokens: lista<Token> = lista_criar();
lista_anexar(tokens, Token.Fim);
nova primeiro: Token = lista_obter(tokens, 0);

nova idades: mapa<verso,bombom> = mapa_criar();
mapa_definir(idades, "ana", 42);
nova idade: bombom = mapa_obter(idades, "ana");
```

Desde a Fase 236, funções de usuário podem declarar parâmetros de tipo explícitos e são monomorfizadas por chamada concreta:

```pink
carinho identidade<T>(valor: T) -> T {
    mimo valor;
}

nova n: bombom = identidade<bombom>(42);
nova s: verso = identidade<verso>("ok");
```

O recorte atual exige chamada explícita `nome<T>(...)`; não há inferência de tipo, generics em `leque`/`ninho` nominais, tipos associados, bounds ou especialização. `T` pode aparecer em parâmetros, retorno, anotações locais e em `lista<T>` no recorte público já suportado.

### `leque`
Enumeração nominal com variantes nomeadas, opcionalmente com cargas (`bombom`, `verso` ou outro leque — inclusive o próprio, permitindo tipos recursivos):

```pink
leque Cor { Vermelho, Verde, Azul }

leque Token { Numero(bombom), Palavra(verso), Fim }

leque Expr { Lit(bombom), Soma(Expr, Expr) }

carinho avalia(e: Expr) -> bombom {
    encaixe e {
        caso Expr.Lit(n) { mimo n; }
        caso Expr.Soma(a, b) { mimo avalia(a) + avalia(b); }
    }
    mimo 0;
}

carinho principal() -> bombom {
    falar(avalia(Expr.Soma(Expr.Lit(2), Expr.Lit(40))));
    mimo 0;
}
```

Dois leques diferentes são tipos distintos mesmo com variantes de mesmo nome. Em leques **sem carga**, a comparação usa `==`/`!=` (inclusive em `escolha`) e o discriminante pode ser lido com `virar bombom`. Em leques **com carga**, a desconstrução acontece exclusivamente via `encaixe`: o compilador exige que todas as variantes sejam cobertas ou que exista um `senao`, e cada `caso Leque.Variante(a, b, ...)` liga as cargas a variáveis novas no corpo do caso, na ordem da declaração.

### `tentar` para resultado estruturado

Desde a Fase 223, a Pinker aceita um primeiro construto de error handling estruturado sobre leques de resultado declarados pelo usuário. O padrão canônico é declarar um leque com uma variante de sucesso e uma variante de falha, ambas com carga, e despachar com `tentar`:

```pink
leque Resultado { Ok(bombom), Erro(verso) }

carinho validar(v: bombom, ok: logica) -> Resultado {
    talvez ok {
        mimo Resultado.Ok(v);
    }
    mimo Resultado.Erro("falha");
}

carinho principal() -> bombom {
    nova r: Resultado = validar(42, verdade);
    tentar r {
        sucesso Resultado.Ok(valor) { falar(valor); }
        falha Resultado.Erro(msg) { falar(msg); }
    }
    mimo 0;
}
```

`sucesso` e `falha` precisam aparecer exatamente uma vez dentro de `tentar`, apontar para variantes do mesmo leque e ligar a mesma quantidade de cargas declaradas na variante. A implementação abaixa para a infraestrutura de leques/`encaixe`, portanto roda no interpretador e no backend nativo.

Desde a Fase 224, funções que retornam o mesmo leque de resultado também podem propagar falhas de forma explícita:

```pinker
propagar validar(42, verdade) como Resultado.Ok(valor) senao Resultado.Erro(msg);
```

`propagar` exige que as duas variantes pertençam ao mesmo leque, sejam distintas e carreguem exatamente um valor. Em caso de falha, a carga é extraída e retornada imediatamente como a variante de falha indicada; em caso de sucesso, desde a Fase 231 a carga de sucesso fica disponível no nome declarado em `Resultado.Ok(valor)` para os comandos seguintes no mesmo bloco.

Desde a Fase 237, a mesma operação também aceita a forma curta quando o leque tem exatamente uma outra variante com uma carga, que passa a ser a falha inferida:

```pinker
propagar? validar(42, verdade) como Resultado.Ok(valor);
```

`propagar?` continua exigindo a variante de sucesso e o nome do valor que segue no fluxo normal. A inferência é local ao leque declarado: se não existir uma única variante de falha possível com uma carga, o programa é rejeitado.

Desde a Fase 240, leques podem declarar parâmetros de tipo explícitos e serem instanciados por alias:

```pinker
leque Resultado<T, E> {
    Ok(T),
    Erro(E),
}

apelido ResultadoBombomVerso = Resultado<bombom, verso>;
```

O alias nomeia uma instância monomorfizada concreta; construtores e `encaixe` usam o alias (`ResultadoBombomVerso.Ok(42)`, `caso ResultadoBombomVerso.Erro(msg)`). Neste recorte, o uso precisa ser explícito por alias e ainda não há inferência, bounds, métodos associados ou integração automática com erros de runtime.


## Funções anônimas não capturantes

Desde a Fase 225, a Pinker aceita literais `carinho` em expressão para callbacks imediatos e pequenos adaptadores sem captura de escopo externo:

```pinker
nova dobrado: bombom = carinho(v: bombom) -> bombom {
    mimo v * 2;
}(21);
```

O literal usa a mesma sintaxe de parâmetros, retorno e bloco de uma função nomeada, mas sem nome entre `carinho` e `(`. A implementação gera uma função sintética top-level e a chamada baixa como chamada direta comum, portanto funciona no interpretador e no backend nativo. Por essa razão, a Fase 225 é deliberadamente **não capturante**: o corpo pode usar seus próprios parâmetros e globais já válidos, mas não pode ler variáveis locais do bloco onde o literal aparece.

Desde a Fase 238, um literal não capturante também pode ser ligado a uma função local tipada e chamado pelo nome local:

```pinker
nova dobrar: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom {
    mimo v * 2;
};

nova valor: bombom = dobrar(21);
```

Desde a Fase 239, funções que recebem parâmetros `carinho(...) -> tipo` podem ser chamadas com funções locais estáticas compatíveis:

```pinker
carinho aplicar(f: carinho(bombom) -> bombom, x: bombom) -> bombom {
    mimo f(x);
}

nova dobrar: carinho(bombom) -> bombom = carinho(v: bombom) -> bombom {
    mimo v * 2;
};

nova valor: bombom = aplicar(dobrar, 21);
```

Neste recorte, a função local continua sendo um alias estático de chamada: ela precisa ser inicializada diretamente por literal `carinho`, não pode ser `muda`, e a passagem como parâmetro especializa estaticamente a função chamada. Retorno de função, armazenamento amplo, closures com ambiente capturado e chamada indireta real continuam para fases posteriores do Eixo A.

## Tratos estáticos e chamada por método

Desde a Fase 226, a Pinker aceita contratos estáticos com `trato`:

```pinker
trato Dobravel {
    carinho dobrar(x: bombom) -> bombom;
}

carinho dobrar(x: bombom) -> bombom {
    mimo x * 2;
}

nova valor: bombom = 21.dobrar();
```

Um `trato` declara assinaturas que precisam existir como funções top-level compatíveis. A chamada `alvo.metodo(a, b)` é açúcar estático para chamada direta com o receiver como primeiro argumento e roda no interpretador e no backend nativo. Esta fase não introduz dicionários/vtables, dynamic dispatch, herança ou objetos de trait; ela cria o contrato estático inicial e a ergonomia de chamada necessária para continuar o item.

Desde as Fases 227–230, `impl Trato para Tipo { ... }` agrupa métodos com receiver explícito, valida cobertura completa do contrato e a chamada `alvo.metodo(...)` resolve primeiro pelo tipo do receiver:

```pinker
impl Dobravel para bombom {
    carinho dobrar(valor: bombom) -> bombom {
        mimo valor + valor;
    }
}
```

O método do `impl` recebe nome interno e não colide com função top-level homônima; se não houver `impl` compatível, a forma antiga por função global ainda é aceita como fallback. O tipo alvo pode ser escalar ou um `ninho` nominal no recorte atual; no backend nativo, `ninho` trafega como parâmetro/local opaco, sem abrir construção por valor nem acesso `p.campo` operacional por valor. Cada `impl` precisa implementar todos os métodos do `trato` e não pode declarar métodos extras. Desde a Fase 232, um mesmo tipo pode implementar múltiplos tratos. Desde a Fase 234, quando tratos diferentes implementados pelo mesmo tipo expõem o mesmo nome de método, `valor.metodo()` é ambíguo e a chamada deve escolher o contrato explicitamente com `Trato.metodo(valor, ...)`.

### Objetos de trato e despacho dinâmico

Desde a Fase 244, um trato objetificável pode ser usado como tipo nominal explícito:

```pink
trato Medivel {
    carinho valor(x: si) -> bombom;
}

impl Medivel para bombom {
    carinho valor(x: bombom) -> bombom { mimo x; }
}

carinho usar(objeto: trato<Medivel>) -> bombom {
    mimo objeto.valor();
}

nova objeto: trato<Medivel> = 42 virar trato<Medivel>;
```

O receiver `si` aparece apenas na assinatura do trato e é substituído pelo tipo concreto no `impl`. A conversão é sempre explícita com `virar`; não há coerção implícita. Nesta fase, a materialização dinâmica aceita tipos concretos escalares e `ninho`; coleções, mapas, arrays, ponteiros, callables, leques e outros tipos aplicados são rejeitados explicitamente, ainda que o sistema estático de `impl` reconheça alguma dessas formas. A materialização copia o valor concreto para um snapshot e produz um handle de uma palavra. O handle aponta para um descritor de duas palavras (`data_ptr`, `vtable_ptr`), e cópias do handle compartilham o descritor. A implementação nativa emite vtables estáticas em `.rodata`, com slots na ordem declarada do trato, e faz chamada indireta pela ABI Linux x86-64 System V com o receiver como primeiro argumento. Métodos que não retornam valor também são suportados.

O interpretador e o backend nativo têm a mesma semântica observável e o exemplo completo está em `examples/fase244_objetos_trato_dinamicos_valido.pink`. O lifetime atual é monotônico: snapshots e descritores não são liberados, coletados nem contados. Coerções, default methods, downcasting/upcasting, herança, igualdade/serialização e objetos de múltiplos tratos continuam fora.

## Ponteiros crus de função

Desde a Fase 245, `seta<carinho(P...) -> R>` representa diretamente um
endereço de código com assinatura concreta. O operador `&` obtém o endereço de
uma função top-level ou de uma especialização genérica concreta e materializada;
a chamada usa a sintaxe ordinária:

```pinker
carinho dobrar(x: bombom) -> bombom { mimo x * 2; }
nova op: seta<carinho(bombom) -> bombom> = &dobrar;
falar(op(21));
```

Aliases diretos e encadeados preservam a assinatura. O valor pode ser copiado,
reatribuído, passado, retornado, escolhido por branch ou ternário, armazenado
antes da chamada e chamado diretamente mesmo quando é resultado de outra
expressão. Aridade, parâmetros e retorno são validados estruturalmente; a ABI
SysV cobre todo o universo público de uma palavra já aceito em chamada direta:
escalares, `logica`, `verso`, listas, mapas, leques, callables,
objetos de trato e ponteiros de dados e de função. A quantidade de argumentos é
geral, com registradores, spills, alinhamento e extensões estreitas.

O ponteiro cru ocupa uma palavra e não possui descritor, ambiente ou argumento
oculto `__env`. Ele é distinto do callable `carinho(...) -> R`, da closure
`{code_ptr, env_ptr}` e do objeto de trato `{data_ptr, vtable_ptr}`: não há
conversões implícitas entre essas categorias. Closures, bound methods sem
receiver explícito, slots de vtable, símbolos sintéticos, especializações não
concretizadas e assinaturas multi-palavra sem ABI estável são rejeitados antes
do lowering.

O literal `0` é o nulo tipado quando o contexto exige ponteiro de função;
qualquer outro inteiro é rejeitado como endereço implícito. Igualdade e
desigualdade aceitam o nulo em qualquer lado e, entre ponteiros vivos, seguem a
identidade de símbolo; ordenação, aritmética e casts inteiro↔ponteiro de função
não fazem parte do contrato. Chamar o nulo termina com diagnóstico
determinístico. O interpretador usa identidade de símbolo
estável, sem endereço do processo hospedeiro; o backend materializa o símbolo
real e usa `call *reg`, sem `__env`.

Seleções ternárias entre ponteiros crus compatíveis preservam a assinatura
comum, tanto quando o resultado é armazenado quanto quando é chamado
imediatamente. Quando uma função direta ou crua retorna `seta<T>`, o lowering
preserva `T` para que cargas e escritas inferidas usem a largura correta.

## Memória explícita

Desde a Fase 246, `alocar(u64) -> seta<u8>` solicita bytes e
`liberar(seta<u8>)` libera exatamente o ponteiro-base uma única vez. A região é
zerada em todos os bytes e tem alinhamento de 16 bytes, suficiente para todas as
classes escalares atualmente acessíveis por `seta<T>`. O maior tamanho aceito é
o que pode ser representado simultaneamente por `u64`, `usize`, `isize` e pelo
layout do runtime; nenhuma conversão trunca. Tamanho zero, overflow, falha do
allocator e tamanho fora dessa interseção terminam com diagnóstico determinístico.

```pinker
nova bytes: seta<u8> = alocar(8);
nova numeros: seta<u64> = bytes virar seta<u64>;
*numeros = 42;
falar(*numeros);
liberar(bytes);
```

Cada alocação pública possui identidade lógica, base, tamanho, alinhamento,
estado `LIVE`/`FREED`, domínio e proveniência. O interpretador modela regiões
esparsas byte a byte, inclusive truncamento e extensão de sinal/zero por
largura; o runtime nativo registra a mesma informação e valida cada load/store
público quanto a vida, limites completos e alinhamento. A arena é monotônica:
uma identidade liberada nunca reutiliza endereço, enquanto as páginas físicas
são descomprometidas. O orçamento é explícito e equivalente nos dois modos:
até 1.000.000 de identidades, 8 GiB de espaço virtual público, metadata
proporcional a esse teto e quarentena física de zero bytes.

Somente um alias do ponteiro-base vivo pode liberar a região, uma vez.
Ponteiro nulo, interior, estrangeiro, estático, de pilha ou pertencente aos
domínios internos de closure, callable, trato e runtime é rejeitado. Cópias e
casts permitidos preservam a identidade; não transferem ownership. Acesso
depois de liberar, desalinhado ou que cruze o último byte é diagnosticado com
paridade entre interpretador e nativo. Cargas assinadas preservam o sinal
também nas comparações relacionais subsequentes.

A validação pública falha fechada quando recebe endereço sem região gerenciada
candidata: sem nenhuma região registrada, nenhum endereço é aceito. O compilador
transporta proveniência pelo lowering e classifica cada ponteiro em **quatro**
classes:

- **`Public`** — região pública conhecida: resultado de `alocar`, de uma chamada
  que devolve `seta<T>`, de um parâmetro de ponteiro, ou derivação desses;
- **`Internal`** — domínio interno reconhecido do próprio runtime, hoje o
  ambiente de closure recebido como parâmetro;
- **`Fabricated`** — endereço construído a partir de um valor **não-ponteiro**,
  tipicamente um inteiro (`<inteiro> virar seta<T>`);
- **`Unclassified`** — ponteiro cuja origem não foi determinada pela análise
  atual. **Não** é sinônimo de inteiro: é ausência de informação sobre a origem,
  não afirmação de que a origem é um inteiro.

Um cast `virar seta<T>` entre tipos de ponteiro só troca o tipo apontado e
**preserva** a classe da origem, incluindo `Unclassified`. `Fabricated` é
produzido quando, e somente quando, um valor não-ponteiro é convertido em
`seta<T>`.

A classificação de uma chamada depende do **tipo de retorno**, nunca da forma da
chamada: chamada direta por símbolo, chamada indireta por valor callable,
chamada por endereço cru de código e chamada de método de trato produzem
`Public` exatamente quando devolvem `seta<T>`. Retorno que não é ponteiro nunca
é `Public`.

Load e store através de ponteiro `Public` ou `Fabricated` passam pela validação,
com o mesmo predicado e sem caminhos divergentes entre os dois sentidos do
acesso. Nesses domínios, endereço recusado termina por **diagnóstico
controlado** com exit 1, não por sinal. `Internal` tem domínio próprio e não é
confrontado com o registro público, porque não é memória pública e validá-lo
rejeitaria acesso legítimo.

Interpretador e nativo **não compartilham implementação de validação**, e o
contrato não afirma isso: o interpretador usa seu modelo de memória sintético e
o nativo chama `pinker_publico_validar_acesso`. O que se garante é que, nos
casos cobertos, os dois apresentam **resultado observável correspondente** —
mesma classe de falha, mesmo diagnóstico e mesmo exit.

Cast de inteiro para ponteiro continua **fora da promessa de segurança de
memória** — a linguagem não passa a garantir que o endereço fabricado é
utilizável. O que vale é mais estreito e verificável: o acesso por endereço
fabricado falha de forma determinística, com diagnóstico
(`E-RUNTIME-MEM-UNKNOWN-ACCESS`) e exit 1, em vez de escrever em memória real e
derrubar o processo por `SIGSEGV`.

**Limite conhecido, e o escopo exato da garantia.** A ausência de término por
sinal de memória vale para acessos por ponteiro classificado como `Public` ou
`Fabricated`. Ela **não** é uma garantia universal sobre todo programa Pinker:
`Unclassified` permanece fora da validação pública enquanto não houver análise
de domínio suficiente, e um acesso por ponteiro dessa classe pode terminar por
sinal. Fechar essa classe exige contrato próprio — tratá-la como exigente foi
testado e rejeita acesso legítimo de closure. Na superfície atual da linguagem
essa classe é estreita: `seta<seta<T>>` não é suportada, a conversão de ponteiro
para inteiro é recusada pela semântica e carga de união não aceita ponteiro,
de modo que um ponteiro não pode ser carregado de memória.

Fica um limite honesto entre os dois modos: o interpretador tem um espaço de
endereços **sintético**, no qual inteiros pequenos podem coincidir com globais
escalares declaradas — `examples/fase71_cast_memoria_valido.pink` é o caso
histórico, válido no interpretador porque o endereço `1` é uma global. O
nativo executa em memória real e não tem esse mapa. Os dois back-ends concordam
em *recusar deterministicamente* endereço não registrado; eles não concordam
sobre *quais* endereços fabricados são válidos, e essa parte é intrínseca ao
modelo de execução, não uma lacuna a fechar.

Os registros de região, vida e próxima identidade pertencem ao estado interno
do interpretador, fora do mapa endereçável pelo programa. Assim, casts de
inteiro para ponteiro não podem observar nem corromper metadata do allocator.

### Cota vitalícia de identidades públicas

O orçamento de **memória** é recuperável; o de **identidade** não. O contrato
medido, idêntico nos dois back-ends, é:

- o limite é de **1.000.000 de identidades públicas por processo**;
- a unidade contada é a **entrada de registro**, isto é, uma chamada de
  `alocar` bem-sucedida — a milionésima passa, a seguinte falha;
- a identidade é consumida no momento da alocação bem-sucedida; toda falha de
  alocação encerra o processo pelo diagnóstico, então não existe caminho
  observável em que uma falha consuma cota;
- `liberar` devolve o armazenamento — as páginas são descomprometidas e o pico
  de memória se estabiliza — mas **não devolve capacidade de identidade**: a
  entrada permanece no registro, marcada como morta;
- o esgotamento **não é recuperável no mesmo processo**, e o diagnóstico é
  estável: `limite de identidades públicas esgotado`, com exit 1.

Um laço de `alocar(1024)` e `liberar` sempre pareados mantém o pico de memória
constante e ainda assim esgota a cota depois de um milhão de ciclos.

A razão é deliberada e vale a pena: a quarentena de identidade é o que permite
distinguir gerações, detectar double free e recusar a revalidação de um alias
obsoleto. Reciclar identidade faria um endereço reutilizado mascarar exatamente
esses erros. Processos de vida longa com muito *churn* de alocação podem esgotar
o orçamento — este é um limite conhecido, não um defeito latente.

Não há reciclagem planejada. Introduzi-la exigiria um contrato próprio de
geração e *lifetime* que hoje não existe; até que exista, a cota é vitalícia.

Uma nota de assimetria, medida e ainda não unificada: no interpretador, o
armazenamento de payload agregado de união também consome uma identidade
pública, porque compartilha o mesmo registro de regiões; no runtime nativo, esse
armazenamento tem orçamento próprio e não toca a cota pública. Programas que
constroem muitas uniões agregadas esgotam a cota mais cedo no interpretador.

A API mantém o modelo fatal estruturado já usado pelas intrínsecas de runtime;
ela não retorna `Resultado<T,E>` porque isso criaria uma segunda ABI de erro
somente para esta intrínseca. Falha controlada existe apenas em builds de teste,
sem superfície pública de produção. Não há ownership estático, GC, RAII,
reference counting ou promessa de memory safety universal: aliases vivos,
liberação no momento correto e vazamentos permanecem responsabilidade do
programa.

## 5) Fluxo de controle

### `talvez` / `senao`

```pink
talvez verdade {
    falar(1);
} senao {
    falar(0);
}
```

### `sempre que`

```pink
nova muda x: bombom = 0;
sempre que x < 3 {
    x = x + 1;
}
```

Observação importante: no `--run`, esse fluxo funciona no subset atual.
No backend externo de `--asm-s` (montável em toolchain C), a Fase 113 abriu loops reais mínimos com condição `==`/`<` no recorte auditável (sem `break`/`continue` amplos e sem comparações gerais).

## 6) Funções e chamadas

Funções usam `carinho`, recebem parâmetros tipados e podem retornar com `mimo`:

```pink
carinho dobro(x: bombom) -> bombom {
    mimo x * 2;
}

carinho principal() -> bombom {
    nova v: bombom = dobro(21);
    mimo v;
}
```

## 6.1) Módulos e imports no estilo canônico atual

No estado atual, a Pinker aceita `trazer modulo;` e `trazer modulo.simbolo;`.
Na apresentação canônica, `trazer` fica logo após `pacote`, um por linha; quando houver mistura, prefira listar primeiro imports de módulo inteiro e depois imports pontuais.

Quando um tipo importado precisa manter origem visível no texto, prefira a forma qualificada já suportada `modulo.Tipo`.
Quando um símbolo já foi trazido pontualmente com `trazer modulo.simbolo;`, prefira a forma curta local para evitar ruído.
Na explicação ao redor do código, não invente alias documental que pareça sintaxe da linguagem: apresente primeiro o nome completo real e só depois use nome curto local quando a origem já estiver clara.

```pink
pacote main;

trazer pessoa_tipos;
trazer pessoa_util.nome_publico;

carinho mostrar_idade(idade: pessoa_tipos.Idade) -> bombom {
    falar(nome_publico(), idade);
    mimo idade;
}
```

Esta convenção é documental e estilística: ela não cria sintaxe nova nem amplia o sistema de módulos.
Ela também não supõe rename de import, alias novo de símbolo ou qualquer forma de `trazer ... como ...`.

## 7) I/O atual

### Saída com `falar`

```pink
falar(42);
falar(verdade);
falar("oi");
```

Todas as variantes de `falar`, incluindo espaços e newline, usam o mesmo writer
no runtime nativo. Falha de saída produz diagnóstico uniforme e exit code 1,
inclusive em pipe fechado.

### Disposição de sinais e pipes fechados

O `main` gerado em modo nativo não passa pela inicialização de runtime da
biblioteca padrão, então a disposição de sinais do processo é estabelecida por
`pinker_rt_iniciar`, **antes da primeira instrução do programa**. `SIGPIPE` é
ignorado, de modo que escrever em pipe fechado devolve `EPIPE` ao runtime e vira
diagnóstico controlado, em qualquer ponto do programa.

Isso é contrato, não detalhe: antes, a disposição era instalada só a partir do
primeiro `falar`. Um programa que escrevia o stdin de um processo filho sem ter
falado antes morria por sinal, com stderr vazio, enquanto o mesmo programa com
um `falar` antes terminava com exit 1 e diagnóstico — e o interpretador dava
exit 1 nos dois casos. O comportamento observável não pode depender da ordem de
execução.

A estratégia é confinada ao processo Pinker: todo filho disparado por
`executar_processo`, `capturar_stdout`, `capturar_stderr`,
`executar_com_entrada` e `pipeline_minimo` recebe `SIGPIPE` restaurado para a
disposição padrão antes do `exec`, e portanto se comporta como qualquer outro
programa da linha de comando.

### Entrada com `ouvir()`

```pink
nova valor: bombom = ouvir();
falar(valor);
```

### Arquivo: `abrir`, `ler_arquivo`, `escrever`, `fechar`

Recorte atual: handle inteiro (`bombom`) sustentado por um descritor aberto.
`abrir` não carrega o conteúdo; leitura, escrita, truncamento e append operam no
mesmo descritor mesmo se o caminho original for renomeado ou removido.
`criar` é exclusivo e nunca trunca entrada existente. Leituras para `verso`
possuem limite explícito de 64 MiB.

```pink
nova h: bombom = abrir("dados.txt");
escrever(h, 123);
nova lido: bombom = ler_arquivo(h);
fechar(h);
falar(lido);
```

## 8) Texto com `verso`

`verso` já é valor operacional no runtime atual (variável local, parâmetro e retorno).

Operações mínimas disponíveis hoje:
- `juntar_verso(a, b)` → concatena dois `verso`;
- `tamanho_verso(v)` → retorna comprimento como `bombom`;
- `formatar_verso(modelo, a[, b])` → monta um `verso` com placeholders sequenciais `{}` e aceita apenas substituições em `bombom` ou `verso`.
- `ler_linha_csv_bombom(linha, sep)` → lê uma única linha CSV mínima em `lista<bombom>` com separador explícito de 1 caractere;
- `emitir_linha_csv_bombom(itens, sep)` → emite uma única linha CSV mínima a partir de `lista<bombom>`.
- `ler_json_plano_bombom(json)` → lê um objeto JSON plano mínimo em `mapa<verso,bombom>`.
- `emitir_json_plano_bombom(mapa)` → emite um objeto JSON plano mínimo a partir de `mapa<verso,bombom>`.
- `tempo_unix()` → retorna o timestamp Unix atual em `bombom`.
- `formatar_tempo_unix(ts)` → formata um timestamp Unix em UTC fixa como `YYYY-MM-DDTHH:MM:SSZ`.
- `executar_processo(comando)` → executa um processo externo mínimo sem shell implícito e retorna o código de saída em `bombom` (na Fase 162, exemplos/testes passaram a usar binários auxiliares do próprio repositório).
- `executar_processo(comando, argv1)` → o mesmo recorte mínimo acima, mas com exatamente um argumento textual explícito adicional, sem shell implícito, sem quoting/escaping rico e sem coleção geral de argv (Fase 168).
- `executar_com_entrada(comando, entrada)` → executa um processo externo mínimo sem shell implícito, envia um único `verso` ao stdin do processo e retorna o código de saída em `bombom`.
- `executar_com_entrada(comando, entrada, argv1)` → o mesmo recorte mínimo acima, mas com exatamente um argumento textual explícito adicional, sem shell implícito, sem quoting/escaping rico e sem coleção geral de argv (Fase 177).
- `pipeline_minimo(produtor, consumidor)` → conecta o stdout textual do primeiro processo ao stdin do segundo, sem shell implícito, sem cadeia longa e retornando apenas o código de saída do consumidor em `bombom`.
- `capturar_stdout(comando)` → executa um processo externo mínimo sem shell implícito e retorna o stdout textual como `verso`, com UTF-8 estrito.
- `capturar_stdout(comando, argv1)` → o mesmo recorte mínimo acima, mas com exatamente um argumento textual explícito adicional, sem shell implícito, sem quoting/escaping rico e sem coleção geral de argv (Fase 169).
- `capturar_stderr(comando)` → executa um processo externo mínimo sem shell implícito e retorna o stderr textual como `verso`, com UTF-8 estrito.
- `capturar_stderr(comando, argv1)` → o mesmo recorte mínimo acima, mas com exatamente um argumento textual explícito adicional, sem shell implícito, sem quoting/escaping rico e sem coleção geral de argv (Fase 170).

Subprocessos não herdam a `PATH` do processo pai para resolução: basenames são
procurados somente em `/usr/local/bin:/usr/bin:/bin`, enquanto comandos com
`/` usam exatamente o caminho informado. `executar_com_entrada` escreve stdin
concorrentemente à espera do filho, fecha o pipe e propaga erros de escrita e
status sem impor timeout oculto.

```pink
nova a: verso = "oi ";
nova b: verso = "Pinker";
nova c: verso = juntar_verso(a, b);
falar(c);
falar(tamanho_verso(c));
falar(formatar_verso("msg={}", c));
```

```pink
nova itens: lista<bombom> = ler_linha_csv_bombom("7,11,13", ",");
falar(lista_bombom_obter(itens, 1));
falar(emitir_linha_csv_bombom(itens, ","));
```

```pink
nova dados: mapa<verso,bombom> = mapa_verso_bombom_criar();
mapa_verso_bombom_definir(dados, "idade", 7);
nova json: verso = emitir_json_plano_bombom(dados);
nova copia: mapa<verso,bombom> = ler_json_plano_bombom(json);
falar(json);
falar(mapa_verso_bombom_obter(copia, "idade"));
```

```pink
nova ts: bombom = tempo_unix();
falar(formatar_tempo_unix(ts));
falar(formatar_tempo_unix(0));
```

```pink
nova codigo: bombom = executar_processo(argumento(0));
falar(codigo);
```

```pink
nova codigo: bombom = executar_processo(argumento(0), "--modo=ok");
falar(codigo);
```

```pink
nova codigo: bombom = executar_com_entrada(argumento(0), "rosa\n");
falar(codigo);
```

```pink
nova codigo: bombom = executar_com_entrada(argumento(0), "argv=ok\n", "--modo=ok");
falar(codigo);
```

```pink
nova codigo: bombom = pipeline_minimo(argumento(0), argumento(1));
falar(codigo);
```

```pink
nova texto: verso = capturar_stdout(argumento(0));
falar(texto);
```

```pink
nova texto: verso = capturar_stdout(argumento(0), "--alvo=rosa");
falar(texto);
```

```pink
nova texto: verso = capturar_stderr(argumento(0));
falar(texto);
```

```pink
nova texto: verso = capturar_stderr(argumento(0), "--alvo=rosa");
falar(texto);
```

Nos exemplos versionados das Fases 162 e 163, o caminho do executável é passado por argv para permitir validação com binários auxiliares do próprio repositório, sem depender de utilitários frágeis do host.

Nos exemplos versionados das Fases 162, 163, 164, 165 e 166, o caminho do executável é passado por argv para permitir validação com binários auxiliares do próprio repositório, sem depender de utilitários frágeis do host.

Limites atuais de texto/dados estruturados/processos: sem slicing de `verso`, sem indexação negativa, sem placeholders nomeados, sem escape rico de chaves, sem quoting complexo de CSV, sem campos multiline, sem CSV geral de múltiplas linhas, sem arrays JSON, sem objetos JSON aninhados, sem escapes ricos em JSON, sem `true`/`false`/`null`, sem timezone configurável, sem locale, sem parser amplo de datas, sem shell implícito, sem argv amplo de subprocesso (as Fases 168, 169, 170 e 177 aceitam apenas um `argv1` textual explícito em `executar_processo`, `capturar_stdout`, `capturar_stderr` e `executar_com_entrada`), sem stdout/stderr combinados, sem redirecionamento rico, sem cadeia longa de pipes e sem stdin interativo/sessão ampla de subprocesso.

## 9) REPL mínimo

O comando `pink repl` abre o primeiro REPL auditável da Pinker.

Recorte real da Fase 167:
- cada linha vira o corpo temporário de `principal`;
- use `falar(...)` para saída textual e `mimo ...;` para retorno explícito;
- `:quit` e `:sair` encerram a sessão;
- não há estado persistente entre linhas;
- não há multiline amplo, histórico sofisticado ou autocomplete.

Exemplo:

```text
$ cargo run --bin pink -- repl
pinker> nova a: bombom = 40; falar(a + 2);
42
ok
pinker> mimo 7;
=> 7
pinker> :quit
Encerrando REPL Pinker.
```

## 10) Exemplos pequenos completos

### A) Somar números

```pink
pacote main;

carinho principal() -> bombom {
    nova a: bombom = 10;
    nova b: bombom = 32;
    mimo a + b;
}
```

### B) Ler entrada e usar valor

```pink
pacote main;

carinho principal() -> bombom {
    nova n: bombom = ouvir();
    falar(n);
    mimo n + 1;
}
```

### C) Usar texto (`verso`)

```pink
pacote main;

carinho principal() -> bombom {
    nova oi: verso = "oi ";
    nova nome: verso = "Pinker";
    nova msg: verso = juntar_verso(oi, nome);
    falar(msg);
    falar(tamanho_verso(msg));
    mimo 0;
}
```

### D) Ler/escrever arquivo

```pink
pacote main;

carinho principal() -> bombom {
    nova h: bombom = abrir("saida.txt");
    escrever(h, 42);
    nova v: bombom = ler_arquivo(h);
    fechar(h);
    falar(v);
    mimo 0;
}
```

## 11) Build nativo (`pink build --nativo`)

Desde a Fase 212 (Eixo B do Bloco 20), além do artefato `.s`, o build pode
produzir um executável nativo real:

```bash
pink build --nativo --out-dir build programa.pink
./build/programa
```

O pipeline emite o `.s`, monta com o driver C do sistema (`cc`/`gcc`/`clang`)
e linka com o runtime nativo `libpinker_rt.a` (construído pelo workspace;
localizável via env `PINKER_RT_LIB`). O corpo do programa ainda está limitado
ao subset do backend `.s`; a paridade completa com o interpretador é o objeto
das fases B2–B11 do Eixo B.

## 11.1) Limites atuais da linguagem

No estado atual, ainda há limites importantes para uso geral:
- o backend nativo alcançou paridade para a superfície versionada compatível do Eixo B, mas ainda há limites fora desse manifesto, como `ouvir` interativo e futuras features de linguagem ainda não abertas;
- error handling estruturado existe via `tentar`, propagação explícita `propagar` e forma curta `propagar?` sobre leques de resultado declarados pelo usuário; tratos estáticos e objetos de trato com despacho dinâmico existem no recorte das Fases 226–230, 232, 234 e 244; funções locais tipadas, valores de função e closures com captura imutável também existem nos recortes versionados; ownership/lifetime de ambientes e objetos continua monotônico e sem desalocação;
- generics cobrem `lista<T>` com `T` = leque e `mapa<K,V>` nas quatro combinações públicas `verso`/`bombom`; funções genéricas de usuário seguem fora;
- API de arquivo segue sem modos avançados de streaming.

## 12) Onde olhar depois

### Assembly inline e uniões estruturais

`sussurro("...");` aceita chunks literais GNU assembler Intel x86-64. O
backend nativo os emite como barreira de efeitos; o interpretador termina com
`E-RUNTIME-SUSSURRO-NATIVO`.

A validação é estrutural, por statements: o scanner divide o texto por newline e
por `;` fora de comentários e regiões citadas e, depois de remover labels e
comentários, recusa todo statement que comece com `.`. Portanto **todas** as
diretivas assembler são rejeitadas por construção, e não por uma lista de nomes.
Labels locais numéricos (`1:`, `jne 1b`) são aceitos; labels nominais não. Cada
bloco é emitido dentro de um envelope com sentinelas geradas pelo compilador, e
o objeto montado é inspecionado para provar que o bloco não criou seção nem
símbolo nomeado adicional.

O prefixo `__pinker_internal_` é reservado ao compilador: qualquer identificador
da fonte que o use — em declaração ou em referência — é recusado com
`E-SEMANTIC-RESERVED-NAMESPACE`.

`uniao<T1, T2, ...>` é estrutural e independente da ordem. A injeção usa
`virar` explícito e a abertura usa `encaixe` com um braço por membro, sem
`senao`. O handle ocupa uma palavra e tem lifetime monotônico nesta fase.

O **valor público** continua sendo esse handle de uma palavra, mas o descritor
guarda um snapshot alinhado do payload **completo**. Cada membro é classificado
como escalar (largura real), handle opaco (uma palavra, cópia rasa por
contrato) ou agregado (`ninho`, array fixo e apelidos resolvidos deles, copiado
byte a byte, incluindo padding). Apelidos são transparentes em profundidade
também na classificação.

Um tipo sem representação de payload conhecida é recusado na semântica, antes da
IR validada, com código estável: `E-SEMANTIC-UNION-PAYLOAD-LAYOUT`,
`E-SEMANTIC-UNION-PAYLOAD-SIZE`, `E-SEMANTIC-UNION-PAYLOAD-ALIGN` ou
`E-SEMANTIC-UNION-PAYLOAD-REPRESENTATION`. Um payload ocupa no máximo 4096
bytes com alinhamento no máximo 16; descritores, bytes de snapshot e metadata
têm orçamentos finitos, revalidados no runtime nativo e no interpretador.

A cópia acontece na injeção: mudar a origem depois não muda o que o `encaixe`
observa. A extração copia para storage novo do binding, de modo que duas
extrações da mesma união não compartilham memória e nenhuma delas expõe o
storage interno do descritor. O comportamento é idêntico no interpretador e no
caminho nativo.

`encaixe` é preservado como construto tipado: o parser guarda o scrutinee e o
tipo de cada braço **como escrito**, sem resolver apelidos e sem calcular tags.
Os apelidos são resolvidos antes da associação dos braços, a cobertura é
validada depois da resolução (dois apelidos do mesmo tipo canônico são o mesmo
membro e são recusados como duplicata) e as tags pertencem exclusivamente ao
registry canônico — o nome do apelido, a ordem dos braços e a ordem textual da
união não definem tag alguma. Ler a tag e abrir o payload são operações internas
tipadas do compilador, não chamadas da linguagem.

A escolha do membro na injeção usa a **identidade semântica resolvida** do tipo
de origem, e não a representação operacional. As duas noções são distintas e não
se substituem: a representação (`TypeIR`, interna ao compilador) diz como o valor
é carregado e armazenado; a identidade resolvida (`ResolvedTypeId`) diz qual tipo
o valor é. Dois `ninho` diferentes, dois `leque` diferentes, duas assinaturas de
`carinho` diferentes e dois `seta<T>` de apontados diferentes compartilham
representação e nunca compartilham identidade. Apelidos são transparentes em
profundidade — `apelido A = Cor` e `seta<A>` resolvem para a identidade de `Cor`
e de `seta<Cor>` —, de modo que o texto do apelido nunca vira identidade.

A identidade acompanha o valor por declarações locais, atribuições, parâmetros,
retornos, chamadas diretas e indiretas, ternários, valores callable, closures,
capturas, extração de payload e reinjeção. A injeção exige igualdade exata de
identidade com um membro do registry e **não** tem desempate por primeira
ocorrência; a tag é copiada desse membro e nenhuma camada posterior reescolhe
membro. Identidade perdida ou ambígua é erro do compilador
(`E-IR-TYPE-IDENTITY-LOST`, `E-IR-UNION-IDENTITY-DUPLICATE`,
`E-IR-UNION-MEMBER-IDENTITY-MISMATCH`), nunca um resultado silencioso.

O interpretador limita a execução a 64 chamadas Pinker simultâneas. O retorno
de `principal` não é impresso: seus 8 bits baixos formam o exit status.

- `docs/style.md` — Norma Visual Oficial Mínima (convenções de estilo e estética).
- `README.md` — visão geral do projeto, modos de execução e comandos.
- `docs/vocabulario.md` — catálogo de keywords da linguagem.
- `docs/roadmap.md` — trilha ativa oficial de implementação.
- `docs/history.md` — histórico oficial de fases, hotfixes e rodadas documentais.
