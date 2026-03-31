# Manual da linguagem Pinker (estado atual)

Este manual ensina a **usar a Pinker hoje**, no recorte que já está implementado no projeto.

## 1) O que é a Pinker

A Pinker é uma linguagem com sintaxe em português e vocabulário próprio (como `carinho`, `mimo`, `talvez`, `verso`).
Neste documento, o foco é prático: escrever programas que funcionam no estado atual do runtime e do frontend.

## 2) Como ler a Pinker

A Pinker usa palavras-chave próprias, então o código pode parecer diferente de outras linguagens à primeira vista.
Use este manual como referência de **uso atual**: aqui só entram construções e recortes que já funcionam hoje.

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

```pink
nova a: verso = "oi ";
nova b: verso = "Pinker";
nova c: verso = juntar_verso(a, b);
falar(c);
falar(tamanho_verso(c));
falar(formatar_verso("msg={}", c));
```

Limites atuais de texto: sem slicing de `verso`, sem indexação negativa, sem placeholders nomeados, sem escape rico de chaves e sem formatação/interpolação avançada.

## 9) Exemplos pequenos completos

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

## 10) Limites atuais da linguagem

No estado atual, ainda há limites importantes para uso geral:
- backend nativo completo ainda não existe;
- backend externo montável ainda é subset mínimo (branch condicional apenas no recorte `==` + `cmp`/`jcc`; sem `sempre que`);
- texto em `verso` ainda está em recorte mínimo (`juntar_verso` e `tamanho_verso`);
- API de arquivo ainda é mínima (sem modos avançados, append ou streaming);
- recursos avançados (generics/traits/enums completos etc.) seguem fora do escopo atual.

## 11) Onde olhar depois

- `README.md` — visão geral do projeto, modos de execução e comandos.
- `docs/vocabulario.md` — catálogo de keywords da linguagem.
- `docs/roadmap.md` — trilha ativa oficial de implementação.
- `docs/history.md` — histórico oficial de fases, hotfixes e rodadas documentais.
