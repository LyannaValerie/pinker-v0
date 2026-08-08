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

## Camadas do núcleo

O núcleo tem duas camadas, e a fronteira entre elas é o filesystem:

| Camada | Módulos | Toca o disco? |
|---|---|---|
| puro | `mod.rs`, `path.rs`, `plan.rs`, `compare.rs`, `report.rs` | não |
| local | `root.rs`, `fsio.rs` | sim, e só ali |

A camada pura continua recebendo o estado observado como **dado**: `check` é
trivialmente sem escrita porque não tem filesystem para escrever. A camada local
é o único ponto em que o disco entra e sai, e ela atravessa sempre o
confinamento.

Fora do núcleo continuam CLI, regras de domínio, processos, rede, Git e qualquer
alteração do contrato congelado `pink-agent-v1`. O primeiro consumidor real é o
lifecycle de snapshots de projeção: ele calcula bytes desejados para preparar e
aceitar candidates, mas delega integralmente ao núcleo root, paths, allowlists,
observação, digest, autorização, stale protection, apply e relatório de
progresso. Essa adoção não amplia a autoridade do core.

## O plano é efêmero

Um plano descreve o estado desejado de um conjunto de arquivos repo-relativos.
Ele é calculado pelo adaptador, usado e descartado: **não é canônico, não é
versionado no repositório e nunca é lido de volta.**

Por isso existe serialização canônica e **não existe parser**. A autorização de
uma escrita compara o digest de um plano recalculado pelo adaptador,
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

<!-- @pinker-doc:start
id: development.automation-core-contract.aplicacao
tags: [desenvolvimento, automacao, apply, atomicidade, confinamento]
aliases:
  - apply do automation core
  - confinamento de paths
  - progresso parcial
summary: Raiz canônica, confinamento no filesystem, autorização por digest, atomicidade por arquivo e o contrato explícito de progresso parcial sem rollback.
-->
## Raiz canônica

A raiz é descoberta subindo do diretório de partida até encontrar
`.pinker/doc.toml` — o mesmo marcador que a Trama já usa como configuração
canônica, não um segundo marcador inventado. O caminho de partida é
canonicalizado antes da subida, de modo que dois caminhos para o mesmo
repositório, um deles por link simbólico, convergem para a mesma raiz.

`RepoRoot::at` aceita uma raiz declarada e **não sobe**: quem declara a raiz
declara exatamente qual é.

## Confinamento

Três portões distintos, nesta ordem:

1. **lexical** — no tipo `RelativePath`, desde o recorte puro;
2. **allowlist** — o target precisa ter sido declarado;
3. **filesystem** — cada ancestral existente e o próprio alvo passam por
   `symlink_metadata`.

O terceiro portão rejeita link simbólico no alvo, link simbólico em qualquer
ancestral, ancestral que exista e não seja diretório, alvo que exista e não seja
arquivo regular, e qualquer resultado fora da raiz canônica — comparada por
componente, não por prefixo textual, porque `/repo-de-outro` tem `/repo` como
prefixo de string.

O confinamento é revalidado **imediatamente antes** da substituição.

### Ausência é `NotFound`, e nada mais

Um componente ou target só conta como ausente quando `symlink_metadata` devolve
`ErrorKind::NotFound`. Qualquer outro erro — permissão negada num ancestral, I/O
do dispositivo, limite de descritores — é `IO_FAILURE` explícito.

A distinção não é cosmética. Se um erro operacional virasse ausência, um target
existente sob um diretório sem permissão de travessia seria observado como
inexistente, e daí sairia `CREATE` — ou, num plano de remoção, `NO_CHANGE` e
portanto `MATCH`: uma automação concluiria que o repositório está convergido
justamente porque não conseguiu olhar para ele. Pelo mesmo motivo, `final_drift`
nunca é `Measured` quando a observação final falha; ele é `Unknown` com a razão.

A mesma regra vale para o temporário: só `ErrorKind::AlreadyExists` justifica
tentar o próximo nome. Qualquer outro erro de criação já vale para todos os
nomes, e insistir nas 64 tentativas apenas esconderia a causa real atrás de uma
mensagem de exaustão.

### O que o confinamento não promete

Não há proteção absoluta contra TOCTOU em filesystem hostil concorrente. A
política é lexical mais `symlink_metadata`, e não substitui `openat2` com
`RESOLVE_BENEATH`.

O runner tem um `ConfinedFs` sobre descritores que faz exatamente isso, mas ele
é privado de `src/agent.rs`, deriva a própria raiz do pai do alvo em vez de uma
raiz de repositório, existe apenas para `linux/x86_64` e vive dentro de uma
região catalogada da superfície congelada `pink-agent-v1`. Torná-lo público
mudaria essa superfície e recalibraria medidas históricas da cartografia.

Este núcleo também **não cria diretórios**: um target cujo diretório pai não
existe falha explicitamente.

## Autorização e plano obsoleto

`apply` recebe uma `Authorization`. O tipo é a prova: "apply sem digest" não é um
erro em tempo de execução, é uma expressão que não compila. O valor autorizado é
comparado com o digest do plano apresentado, e qualquer divergência para antes de
tocar o disco.

Antes de escrever, o núcleo reobserva e compara com as precondições registradas
no check. Se o estado observado de qualquer target mudou, o resultado é
`STALE_PLAN` e nada é escrito. O recálculo do estado desejado é do **adaptador**:
o núcleo não sabe derivar o plano, então valida o que recebe em vez de fingir que
recalculou.

## Atomicidade por arquivo

Para cada target, nesta ordem: temporário irmão criado com `create_new`, escrita
completa, sync quando suportado, revalidação do confinamento, `rename` no mesmo
diretório, sync do diretório quando suportado, releitura verificando **tamanho e
digest**.

O temporário é exclusivo por construção; havendo colisão, tenta-se o próximo
nome, até um limite explícito. Falha em qualquer ponto anterior ao `rename`
remove o temporário e preserva o alvo intacto.

A garantia real por arquivo é a releitura, não o `sync`.

## Progresso parcial

Não há atomicidade multi-arquivo e não há rollback global. Um relatório de
aplicação carrega sempre:

| Campo | Significado |
|---|---|
| `applied` | targets aplicados e verificados |
| `failed` | o target em que parou |
| `not_attempted` | os que nem chegaram a ser tentados |
| `rollback_performed` | sempre `false` |
| `final_drift` | medido, ou `UNKNOWN` **com a razão** |
| `failure` | a causa |
| `decision` | `NEEDS_HUMAN_DECISION`, ao lado da causa |
| `recovery` | o procedimento, impresso |

Aplicação parcial não é `APPLIED`: o outcome fica ausente e a causa ocupa seu
lugar próprio. A recuperação é observar novamente, executar novo check, produzir
novo plano e autorizar novo digest — não há retry cego e não há reaplicação de
plano obsoleto.
<!-- @pinker-doc:end development.automation-core-contract.aplicacao -->
