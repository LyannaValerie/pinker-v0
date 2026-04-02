# Norma Visual Oficial Mínima (Pinker v0)

Esta é a primeira norma visual oficial da Pinker. Ela define a estética preferida para documentos, exemplos e código canônico. 

**Importante:** Esta norma é uma convenção de estilo e legibilidade. Ela não altera a gramática da linguagem nesta fase. O parser continua aceitando variações que respeitem a sintaxe técnica, mas o código "oficial" deve seguir estas diretrizes.

---

## 1. Organização do Topo e Imports

- **Pacote:** A declaração `pacote ...;` deve ser a primeira linha do arquivo.
- **Imports:** Declarações `trazer` devem vir logo após o pacote, uma por linha.
- **Espaçamento:**
  - 1 linha em branco entre `pacote` e o primeiro `trazer`.
  - 1 linha em branco entre o último `trazer` e a primeira declaração global (função, ninho, etc.).
- **Agrupamento mínimo:** quando houver mistura de `trazer modulo;` e `trazer modulo.simbolo;`, apresentar primeiro os imports de módulo inteiro e depois os imports pontuais de símbolo.
- **Separação interna:** em listas curtas, não abrir linhas em branco entre `trazer`; em listas maiores, usar no máximo uma linha em branco para separar esses dois grupos.

Exemplo preferido:
```pink
pacote main;

trazer pessoa;
trazer pessoa_tipos;

trazer pessoa.nome_publico;
trazer pessoa.IDADE_PADRAO;

carinho principal() -> bombom {
    ...
}
```

## 1.1 Uso qualificado em documentação canônica

- **Natureza da regra:** esta é uma convenção de apresentação para docs e exemplos canônicos; não é sintaxe nova nem obrigação semântica adicional.
- **Quando preferir:** use forma qualificada já suportada quando ela reduzir ambiguidade de origem, sobretudo para tipos importados em contexto tipado, como `modulo.Tipo`.
- **Quando evitar excesso:** se o símbolo já foi importado pontualmente com `trazer modulo.simbolo;`, prefira a forma curta local nos exemplos para evitar ruído visual repetitivo.
- **Escopo atual implementado:** nesta fase, a convenção cobre apenas as formas já aceitas hoje pelo projeto, como `trazer modulo;`, `trazer modulo.simbolo;` e `modulo.Tipo` em contexto tipado.

Exemplo:
```pink
pacote main;

trazer util;
trazer rede;

carinho principal() -> bombom {
    ...
}
```

## 2. Assinaturas e Declarações

- **Espaço após dois-pontos:** Sempre use um espaço após `:` em declarações de tipos.
- **Seta de retorno:** Use espaços ao redor de `->`.
- **Palavras-chave:** Use um espaço após `carinho`, `nova`, `muda`.

Preferido:
```pink
carinho somar(a: bombom, b: bombom) -> bombom
nova muda contador: bombom = 0;
```

Evitar:
```pink
carinho somar(a:bombom,b:bombom)->bombom
nova muda contador:bombom=0;
```

## 3. Blocos e Espaçamento Visual

- **Indentação:** 4 espaços (não use tabs).
- **Abertura de bloco:** O `{` deve estar na mesma linha da instrução que o inicia.
- **Fechamento de bloco:** O `}` deve estar em sua própria linha, alinhado com o início da instrução.
- **Linhas em branco:**
  - Use exatamente 1 linha em branco para separar funções ou definições de `ninho`.
  - Evite linhas em branco no início ou no final de blocos `{ ... }`.
  - Use linhas em branco dentro de funções apenas para separar blocos lógicos complexos.

## 4. Fluxo de Controle

- **talvez / senao:** O `senao` deve ficar na mesma linha do fechamento do bloco `talvez`.

```pink
talvez condicao {
    ...
} senao {
    ...
}
```

- **sempre que:**
```pink
sempre que condicao {
    ...
}
```

## 5. Comentários e Documentação

- **Tom:** Use um tom profissional, direto e coerente com o vocabulário da Pinker.
- **Posição:** Prefira comentários em linhas próprias acima do código que explicam, ou ao final da linha para notas curtas.

## 6. Resumo da Estética Canônica

O objetivo da norma visual é reduzir a "cacofonia documental" e garantir que a Pinker pareça uma linguagem madura e organizada, mesmo usando um vocabulário lúdico.

**O que não mudou:**
- A sintaxe `;` continua obrigatória onde já era.
- O parser não impõe essas regras de estilo (ainda).
- A compatibilidade funcional é 100% preservada.
