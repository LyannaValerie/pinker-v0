---
pinker-doc: 1
id: development.automation-core-contract
domain: development
kind: reference
status: active
parent: development
audience:
  - human
  - agent
canonical_for:
  - development.automation-core-contract
related:
  - development.deterministic-infrastructure-window
  - development.projection-snapshots-contract
  - development
---

# Contrato do núcleo comum de automação

- **Classe:** Engine
- **Papel:** contrato de infraestrutura
- **Status:** ativo

Este documento descreve o **automation core** implementado em `src/automation/`
sob a terceira capacidade da janela auxiliar
(`janela-infraestrutura-deterministica.md`, Issue #385).

<!-- @pinker-doc:start
id: development.automation-core-contract.fronteira
tags: [desenvolvimento, automacao, determinismo, plano, drift]
aliases:
  - automation core
  - nucleo comum de automacao
  - plano de automacao
summary: Fronteira entre pink agente, adaptador de domínio e núcleo, e o que o núcleo puro pode e não pode fazer.
-->
## Fronteira

```text
pink agente          → orquestração, processos, Git, rede e publicação
adaptador de domínio → invariantes e cálculo do estado desejado
automation core      → plano, comparação, check local e relatórios
```

O núcleo não executa processos, não acessa a rede, não executa Git, não publica
e não conhece estados internos do runner nem do `pink agente`. Sua única
dependência sobre `src/agent.rs` é `sha256_hex`, função pura de bytes: o
contrato proíbe adicionar crate de hash e proíbe duplicar hashing em silêncio, e
essa é a única implementação pública de SHA-256 do repositório.

## O que este recorte não faz

Este é o recorte **puro e somente leitura**. Fora dele ficam filesystem,
descoberta de root, temporários, rename, apply, CLI, consumidor real e qualquer
alteração do contrato congelado `pink-agent-v1`.

O estado observado entra como **dado**, fornecido pelo chamador. A consequência
é que o check aqui é trivialmente sem escrita: não há caminho de escrita a
evitar, porque não há filesystem.

## O plano é efêmero

Um plano descreve o estado desejado de um conjunto de arquivos repo-relativos.
Ele é calculado pelo adaptador, usado e descartado: **não é canônico, não é
versionado no repositório e nunca é lido de volta.**

Por isso existe serialização canônica e **não existe parser**. A autorização de
uma escrita futura compara o digest de um plano recalculado pelo adaptador,
jamais um plano desserializado; não escrever o parser elimina uma superfície
inteira de entrada para um formato que nunca é persistido.

## Forma canônica e digest

JSON de uma linha, ordem de chaves fixa, targets ordenados por path, payload em
hexadecimal minúsculo e remoção representada por `null`:

```json
{"schema":1,"producer":"adaptador","targets":[{"path":"docs/a.md","desired":"6f6c61"},{"path":"docs/b.md","desired":null}]}
```

Só entram paths repo-relativos: nenhum root absoluto alcança esta forma. O
digest é o SHA-256 desses bytes exatos, de modo que **o payload fica coberto
pelo digest** — alterar um único byte de conteúdo muda o digest.

## Limites explícitos

| Constante | Valor | Significado |
|---|---:|---|
| `MAX_TARGET_BYTES` | 8 MiB | bytes decodificados por target |
| `MAX_PLAN_BYTES` | 32 MiB | bytes decodificados somados no plano |
| `MAX_PATH_LEN` | 512 | comprimento de um path repo-relativo |

São conservadores e explícitos, e cobertos por teste no limite e um byte acima,
para que qualquer alteração futura seja deliberada.
<!-- @pinker-doc:end development.automation-core-contract.fronteira -->

<!-- @pinker-doc:start
id: development.automation-core-contract.resultados
tags: [desenvolvimento, automacao, outcomes, falhas, relatorios]
aliases:
  - outcomes da automacao
  - falhas operacionais da automacao
  - relatorio de automacao
summary: Classificação de mudanças, outcomes de domínio, falhas operacionais separadas e o que os relatórios podem carregar.
-->
## Classificação

A comparação é de bytes, e a classificação tem quatro formas:

| Desejado | Observado | Classificação |
|---|---|---|
| presente | ausente | `CREATE` |
| presente | diferente | `REPLACE` |
| presente | igual | `NO_CHANGE` |
| ausente | presente | `REMOVE` |
| ausente | ausente | `NO_CHANGE` |

Conteúdo vazio e ausência são coisas distintas, tanto na classificação quanto no
digest.

## Outcomes e falhas

Resultados de domínio: `MATCH`, `DRIFT`, `APPLIED` e `NO_CHANGE`.

Falhas operacionais, separadas de propósito: `HARNESS_FAILURE`,
`POLICY_VIOLATION`, `STALE_PLAN`, `IO_FAILURE` e `VERIFY_AFTER_APPLY_FAILURE`.

`NEEDS_HUMAN_DECISION` é estado decisório e **nunca substitui a causa**: um
relatório de falha carrega as duas coisas lado a lado.

Drift não é erro, e falha de harness nunca vira drift — a separação é de tipo,
não de convenção: o check devolve `Result`, então uma falha não pode ocupar o
lugar de um outcome.

Neste recorte, o núcleo puro produz operacionalmente apenas `MATCH`, `DRIFT`,
`HARNESS_FAILURE` e `POLICY_VIOLATION`. Os demais existem no modelo para que o
schema seja estável quando o apply chegar, e não são simulados.

## Política de paths

Este estágio valida apenas a **forma** do path: rejeita vazio, absoluto,
travessia por `..`, componente degenerado (`.` ou vazio), barra invertida,
caractere de controle e excesso de comprimento. A allowlist é lógica e em
memória: ela responde "este target foi declarado?".

O confinamento real — descoberta canônica do root, resolução no filesystem e
rejeição de symlink no target e em ancestral — pertence ao estágio de apply e
depende de syscalls que o núcleo puro não executa. Um path lexicalmente válido
ainda pode ser inseguro no disco, e prometer o contrário aqui seria enganoso.

## Relatórios

JSON de máquina e Markdown derivado do **mesmo modelo**, para que não possam
divergir. Nenhum dos dois carrega o payload completo — um relatório descreve o
que mudaria, não o conteúdo — e nenhum carrega root absoluto, porque o modelo só
conhece paths repo-relativos. Não há códigos ANSI no JSON.
<!-- @pinker-doc:end development.automation-core-contract.resultados -->
