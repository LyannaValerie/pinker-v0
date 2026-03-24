# Arquitetura lexical da Pinker

- **Classe:** Rosa
- **Papel:** léxico
- **Status:** ativo

Este documento define o eixo lexical da Pinker de forma arquitetural.

## 1. Critérios de keyword forte

Uma keyword é forte quando, ao mesmo tempo:

1. **Nomeia bem a função técnica** (sem ambiguidade operacional).
2. **Carrega tom da linguagem** (gentil, firme, sem infantilização).
3. **É sustentável no ecossistema** (cabe em docs, erros, tooling e ensino).
4. **Escala semanticamente** (não quebra quando o recurso amadurece).
5. **Mantém diferenciação identitária** sem sacrificar legibilidade.

## 2. Sinais de keyword ruim

Evitar keywords que:

- exigem explicação longa para tarefa simples;
- parecem piada ou caricatura do projeto;
- competem semanticamente com palavras já estabelecidas;
- são tecnicamente corretas, mas descaracterizam o tom Pinker;
- funcionam só no recorte atual e colapsam em versões futuras.

## 3. Famílias lexicais fortes da Pinker

- **Estruturas centrais:** `pacote`, `carinho`, `mimo`, `principal`.
- **Fluxo e controle:** `talvez`, `senao`, `sempre que`, `quebrar`, `continuar`.
- **Estado e tipo:** `nova`, `muda`, `apelido`, `ninho`, `seta`, `fragil`, `virar`.
- **Texto e expressão:** `verso`, `falar`, `juntar_verso`, `tamanho_verso`.
- **I/O e tooling mínimo:** `ouvir`, `abrir`, `fechar`, `escrever`, `argumento`, `sair`.

## 4. Keywords aceitas e implementadas

### Núcleo de linguagem

`pacote`, `carinho`, `mimo`, `talvez`, `senao`, `sempre que`, `quebrar`, `continuar`, `eterno`, `nova`, `muda`, `bombom`, `logica`, `verdade`, `falso`, `principal`.

### Sistema de tipos e memória

`apelido`, `ninho`, `seta`, `virar`, `peso`, `alinhamento`, `fragil`, `sussurro`, `livre`, `trazer`, `verso`.

### Runtime textual/I/O (Bloco 8)

`falar`, `ouvir`, `abrir`, `fechar`, `escrever`, `ler_arquivo`, `juntar_verso`, `tamanho_verso`, `indice_verso`, `argumento`, `argumento_ou`, `quantos_argumentos`, `tem_argumento`, `ambiente_ou`, `diretorio_atual`, `caminho_existe`, `e_arquivo`, `e_diretorio`, `juntar_caminho`, `tamanho_arquivo`, `e_vazio`, `criar_diretorio`, `remover_arquivo`, `remover_diretorio`, `ler_verso_arquivo`, `criar_arquivo`, `escrever_verso`, `truncar_arquivo`, `contem_verso`, `comeca_com`, `termina_com`, `igual_verso`, `vazio_verso`, `aparar_verso`, `minusculo_verso`, `maiusculo_verso`, `sair`.

### Forma textual dual adicional

`nope` (equivalente textual de `~`).

## 5. Keywords rejeitadas (decisão atual)

- `ninhozinho` e diminutivos em geral: quebra de tom técnico.
- `instavel` para volatile: perde nuance de cuidado ativo de `fragil`.
- `tornar` como cast principal: menos preciso que `virar` no uso atual.

## 6. Keywords provisórias (aceitáveis, ainda não fechadas)

- Tipos/estrutura: `leque`, `par`, `canto`, `letra`, `grao`.
- Controle: `passeio`, `encaixe`, `roda`.
- Erros/fluxo: `amparo`, `tropeco`.
- Abstração: `molde`, `vestir`, `qualquer`.
- Sistemas: `reserva`, `soltar`, `raiz`.

## 7. Keywords tecnicamente aceitáveis, mas fracas identitariamente

- `publico` / `privado` (claras, porém genéricas para o tom Pinker).
- `modulo` (funcional, porém menos característico que `canto`).
- `unsafe` transliterado (técnico, mas desalinhado à proposta de voz).

## 8. Vocabulário técnico x final x provisório

- **Técnico:** termos de implementação/documentação de engine (`volatile`, `cast`, `alias`, `backend`).
- **Final (Pinker):** superfície pública da linguagem (`fragil`, `virar`, `apelido`).
- **Provisório:** hipótese lexical ainda em avaliação para futuras features.

Regra prática: termo técnico pode aparecer na documentação interna, mas a superfície da linguagem deve priorizar o vocabulário final Pinker.

## 9. Governança lexical mínima

Antes de aceitar keyword nova:

1. validar critério técnico + identitário;
2. avaliar colisão com família lexical já existente;
3. registrar status (aceita/rejeitada/provisória);
4. atualizar `docs/history.md` quando houver decisão material.
