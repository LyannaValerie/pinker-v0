# Concha — a shell escrita em Pinker

Um interpretador de linha de comando no espírito do `bash`, escrito **inteiramente
em Pinker**: nenhuma linha de Rust foi adicionada ao compilador para ele existir, e
nenhum comando passa por `sh -c`. Cada programa externo é um spawn direto, com argv
de verdade, diretório de trabalho próprio e overlay de ambiente.

O que a concha **não** consegue fazer — e por quê, com repro e prova no fonte da
própria Pinker — está em [`LIMITES.md`](LIMITES.md).

## Comando

```bash
# uma linha só
cargo run --bin pink -- --run apps/concha/principal.pink -- --comando 'ls | wc -l'

# roteiro
cargo run --bin pink -- --run apps/concha/principal.pink -- --roteiro apps/concha/demo.concha

# interativo (REPL)
cargo run --bin pink -- --run apps/concha/principal.pink
```

## Contrato de resultado

- sem `--comando` e sem `--roteiro`: abre o REPL e lê da entrada padrão até `sair` ou fim de entrada;
- o status devolvido é o do último comando executado, ou o argumento de `sair N`;
- diagnósticos da própria concha saem prefixados com `concha:`.

## O que já funciona

| Área | Superfície |
|---|---|
| Palavras | aspas simples e duplas, escape com `\`, comentário `#` |
| Expansão | `$NOME`, `${NOME}`, `$?`, `~`, substituição de comando `$(...)` (aninhável) |
| Glob | `*` e `?` no último componente, com ordenação feita pela concha |
| Redirecionamento | `>`, `>>`, `<`, `2>` |
| Pipeline | `a \| b \| c` com quantos estágios quiser |
| Encadeamento | `;`, `&&`, `\|\|`, status em `$?` |
| Atribuição | `NOME=valor` (inclusive como prefixo de comando) |
| Controle de fluxo | `se … entao … senao … fim`, `enquanto … faca … fim`, `para X em … faca … fim` |
| Builtins | `cd` (com `cd -`), `pwd`, `echo` (com `-n`), `export`, `unset`, `set`, `ler`, `type`, `fonte`, `ajuda`, `versao`, `sair`, `:`, `true`, `false` |
| Externos | spawn direto com argv, cwd e ambiente exportado |

## Como ela roda por dentro

```
linha  →  dividir_em_segmentos  →  tokenizar (aspas, $, glob)  →  executar_bloco
                                                                 ├── construtos (se/enquanto/para)
                                                                 └── rodar_pipeline
                                                                      └── rodar_estagio
                                                                           ├── builtin
                                                                           └── executar_processo_estruturado
```

O estado (diretório atual, último status, variáveis, exportadas) vive em
`mapa<verso,verso>`, que é handle na Pinker e por isso é observado por todas as
funções que o recebem — a linguagem ainda não tem estado global mutável.

## Backend nativo

A concha inteira compila para binário x86-64 real, com uma única exceção: a família
`ouvir*` (leitura da entrada padrão) não tem símbolo no runtime nativo. Ou seja, dá
para ter uma shell nativa que executa roteiros e não sabe ler o que você digita:

```bash
sed 's/ouvir_verso_ou(sentinela_fim())/sentinela_fim()/; s/ouvir_verso_ou("")/""/' \
  apps/concha/principal.pink > /tmp/concha_sem_repl.pink
PINKER_RT_LIB=target/release/libpinker_rt.a \
  cargo run --bin pink -- build --nativo --out-dir /tmp/concha_nat /tmp/concha_sem_repl.pink
/tmp/concha_nat/concha_sem_repl --roteiro apps/concha/demo.concha
```

Detalhes e prova em [`LIMITES.md`](LIMITES.md), limite L-17.
