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

Neste recorte, a função local é um alias estático de chamada: ela precisa ser inicializada diretamente por literal `carinho`, não pode ser `muda` e ainda não é valor passável/retornável. Tipos-função públicos para parâmetros/retorno, passagem de função como argumento e closures com ambiente capturado continuam para fases posteriores do Eixo A.

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

O método do `impl` recebe nome interno e não colide com função top-level homônima; se não houver `impl` compatível, a forma antiga por função global ainda é aceita como fallback. O tipo alvo pode ser escalar ou um `ninho` nominal no recorte atual; no backend nativo, `ninho` trafega como parâmetro/local opaco, sem abrir construção por valor nem acesso `p.campo` operacional por valor. Cada `impl` precisa implementar todos os métodos do `trato` e não pode declarar métodos extras. Desde a Fase 232, um mesmo tipo pode implementar múltiplos tratos. Desde a Fase 234, quando tratos diferentes implementados pelo mesmo tipo expõem o mesmo nome de método, `valor.metodo()` é ambíguo e a chamada deve escolher o contrato explicitamente com `Trato.metodo(valor, ...)`. Objetos de trait, vtables, dynamic dispatch, coerções, default methods e overloading amplo continuam fora.

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

### Entrada com `ouvir()`

```pink
nova valor: bombom = ouvir();
falar(valor);
```

### Arquivo: `abrir`, `ler_arquivo`, `escrever`, `fechar`

Recorte atual: handle inteiro (`bombom`) e conteúdo tratado como `bombom`.

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
- error handling estruturado existe via `tentar`, propagação explícita `propagar` e forma curta `propagar?` sobre leques de resultado declarados pelo usuário; tratos estáticos, chamada por método e `impl` nominal com cobertura completa existem no recorte das Fases 226–230; funções locais tipadas não capturantes existem como alias estático de chamada; closures capturantes e dynamic dispatch seguem fora;
- generics cobrem `lista<T>` com `T` = leque e `mapa<K,V>` nas quatro combinações públicas `verso`/`bombom`; funções genéricas de usuário seguem fora;
- API de arquivo segue sem modos avançados de streaming.

## 12) Onde olhar depois

- `docs/style.md` — Norma Visual Oficial Mínima (convenções de estilo e estética).
- `README.md` — visão geral do projeto, modos de execução e comandos.
- `docs/vocabulario.md` — catálogo de keywords da linguagem.
- `docs/roadmap.md` — trilha ativa oficial de implementação.
- `docs/history.md` — histórico oficial de fases, hotfixes e rodadas documentais.
