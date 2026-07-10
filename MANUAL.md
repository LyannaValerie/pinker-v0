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

### `leque`
Enumeração nominal com variantes nomeadas (recorte mínimo, sem dados por variante):

```pink
leque Cor { Vermelho, Verde, Azul }

carinho principal() -> bombom {
    nova escolhida: Cor = Cor.Verde;
    escolha escolhida {
        caso Cor.Vermelho { falar("quente"); }
        senao { falar("fria"); }
    }
    mimo 0;
}
```

Dois leques diferentes são tipos distintos mesmo com variantes de mesmo nome; a comparação usa `==`/`!=` e o discriminante inteiro pode ser lido com `virar bombom`.

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

## 11) Limites atuais da linguagem

No estado atual, ainda há limites importantes para uso geral:
- backend nativo completo ainda não existe;
- backend externo montável ainda é subset mínimo (branch condicional apenas no recorte `==` + `cmp`/`jcc`; sem `sempre que`);
- texto em `verso` ainda está em recorte mínimo (`juntar_verso` e `tamanho_verso`);
- API de arquivo ainda é mínima (sem modos avançados, append ou streaming);
- recursos avançados (generics/traits/enums completos etc.) seguem fora do escopo atual.

## 12) Onde olhar depois

- `docs/style.md` — Norma Visual Oficial Mínima (convenções de estilo e estética).
- `README.md` — visão geral do projeto, modos de execução e comandos.
- `docs/vocabulario.md` — catálogo de keywords da linguagem.
- `docs/roadmap.md` — trilha ativa oficial de implementação.
- `docs/history.md` — histórico oficial de fases, hotfixes e rodadas documentais.
