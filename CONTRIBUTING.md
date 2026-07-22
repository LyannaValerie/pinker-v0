# Contribuindo com a Pinker v0

Obrigado pelo interesse em contribuir. A Pinker v0 é a base factual da linguagem:
um frontend Rust com interpretador, IRs auditáveis, backend e docs versionadas.

Este guia é a porta de entrada para contribuições humanas externas. Comece pelo
tipo de contribuição abaixo; você não precisa ler toda a arquitetura documental
para corrigir algo pequeno. Agentes seguem os contratos próprios indicados em
[Agentes de IA](#agentes-de-ia).

## Escolha o caminho

| Quero... | Começo por... | Coordenação prévia |
|---|---|---|
| corrigir typo, gramática ou clareza local | [Issue de documentação](https://github.com/LyannaValerie/pinker-v0/issues/new?template=documentation.yml), identificando caminho, trecho exato e correção proposta | mantenha o recorte estritamente editorial |
| corrigir comando, link ou afirmação factual | [Issue de documentação](https://github.com/LyannaValerie/pinker-v0/issues/new?template=documentation.yml) para relatar; ao preparar uma correção, preserve a fonte factual no PR | peça orientação se a fonte factual estiver ambígua |
| relatar comportamento reproduzível | [Issue de bug](https://github.com/LyannaValerie/pinker-v0/issues/new?template=bug.yml) | não precisa preparar a correção |
| implementar correção de código já delimitada | uma Issue aceita; comente nela a abordagem pretendida | confirmação da manutenção para escopo incerto ou ambicioso |
| propor linguagem, tooling ou direção | [Discussions Ideas](https://github.com/LyannaValerie/pinker-v0/discussions/categories/ideas) | a Discussion explora a proposta; não autoriza implementação |
| relatar vulnerabilidade | [relato privado](https://github.com/LyannaValerie/pinker-v0/security/advisories/new) | nunca use Issue pública |

Em qualquer caminho, siga o [Código de Conduta](CODE_OF_CONDUCT.md), mantenha o
menor escopo auditável e relate somente evidências e validações reais. Consulte
[SUPPORT.md](SUPPORT.md) se ainda houver dúvida sobre o canal.

## Leitura conforme o recorte

- **Sempre antes de alterar comportamento:** [README.md](README.md), para o
  estado público, e a Issue aceita que delimita o trabalho.
- **Somente para direção ou mudança funcional:** [docs/roadmap.md](docs/roadmap.md),
  [docs/handoff_codex.md](docs/handoff_codex.md) e [GOVERNANCE.md](GOVERNANCE.md).
- **Somente ao alterar documentação:** a seção aplicável de
  [docs/doc_rules.md](docs/doc_rules.md); use [docs/atlas.md](docs/atlas.md) para
  localizar a fonte canônica quando a correção não for apenas local.
- **Somente para vulnerabilidade:** [SECURITY.md](SECURITY.md).
- **Somente para agentes:** [AGENTS.md](AGENTS.md) e as instruções específicas da
  integração. Pessoas contribuindo manualmente não precisam seguir o fluxo de
  inspeção destinado a agentes.

O código e os testes mergeados são a primeira fonte factual. O roadmap define a
ordem ativa; o handoff registra o estado operacional; o histórico preserva a
crônica. [docs/future.md](docs/future.md) é inventário técnico, não backlog nem
autorização automática para implementar um item.

Antes de uma mudança funcional ou de direção, recomenda-se coordenar o recorte
com a manutenção do projeto em [Discussions](https://github.com/LyannaValerie/pinker-v0/discussions)
ou pelos [formulários de Issue](https://github.com/LyannaValerie/pinker-v0/issues/new/choose).
Isso evita furar a ordem ativa ou trabalhar sobre
uma proposta que ainda é apenas horizonte. Correções documentais simples devem
continuar pequenas e factuais.

## Onde encontrar trabalho

O painel dinâmico [Onde contribuir na Pinker](https://github.com/LyannaValerie/pinker-v0/discussions/372)
reúne buscas de trabalho pronto, propostas que ainda precisam de design, itens
bloqueados e o foco corrente. Ele é uma superfície de descoberta: somente
Issues abertas com contrato completo formam o backlog executável; roadmap e
`docs/future.md` não autorizam implementação.

Os rótulos classificam tamanho e estado:

- `good first issue`: recorte pequeno para primeira contribuição;
- `community`: contribuição intermediária delimitada;
- `ambitious` + `design accepted`: trabalho grande cuja fronteira de design foi aceita;
- `needs design` e `discussion-needed`: decisão ainda aberta;
- `blocked`: dependência registrada impede avanço;
- `needs triage`: contrato ou evidência ainda incompletos;
- `help wanted`: trabalho aberto à contribuição externa.

Antes de começar, comente na Issue a abordagem pretendida. Trabalho ambicioso
ou incerto exige confirmação de escopo da manutenção. Isso não cria reserva
exclusiva nem garantia de merge. Contribuições grandes são bem-vindas quando a
fronteira de design foi aceita; propostas novas começam em
[Discussions](https://github.com/LyannaValerie/pinker-v0/discussions), não como
implementação antecipada.

## Trabalho paralelo e dependências

O roadmap registra direção, dependências e ordem de integração; não é uma fila
serial universal. Múltiplas Issues aceitas podem avançar ao mesmo tempo quando
cada uma declara dependências, estado de integração, subsistemas afetados,
compatibilidade, coordenação e critérios de conclusão. Engenharia independente
pode prosseguir enquanto a Founder continua a Trama.

Mudanças em raízes do scanner, marcadores, schemas, catálogos gerados ou
contratos de CLI da Trama permanecem coordenadas na trilha da Trama. Trabalho
grande exige design aceito, marcos e fronteiras de compatibilidade. Comentar uma
Issue não reserva ownership permanente; a ordem de implementação não garante a
ordem de merge, e o merge manual da manutenção permanece a autoridade final.

## Fluxo por fork e pull request

Para contribuir sem acesso de escrita ao repositório, faça um fork, trabalhe em
uma branch do seu fork e abra um pull request contra `LyannaValerie/pinker-v0`.
Um fluxo local possível é:

```bash
git clone https://github.com/SEU_USUARIO/pinker-v0.git
cd pinker-v0
git remote add upstream https://github.com/LyannaValerie/pinker-v0.git
git fetch upstream
git switch -c nome-descritivo upstream/main
```

`nome-descritivo` é apenas um exemplo, não uma convenção obrigatória. Antes de
alterar arquivos, confirme que a branch parte do estado esperado e que o
worktree não contém mudanças alheias ao recorte.

## Ambiente e toolchain

O workspace usa Rust stable `1.78.0`, fixado em [rust-toolchain.toml](rust-toolchain.toml),
com `rustfmt` e `clippy`. A suíte oficial é stable-only: não dependa de nightly
nem de `-Z unstable-options`.

O compilador raiz e o runtime `pinker_rt` não declaram dependências externas.
Preserve essa disciplina salvo uma decisão explícita e sustentada pelo projeto.
Os comandos oficiais passam por `./ci_env.sh`, diretamente ou pelos alvos do
`Makefile`, para sanear flags externas e manter o ambiente reproduzível.

## Como localizar o que mudar

Evite varrer `docs/`, `src/`, `examples/` ou `tests/` sem necessidade:

1. use `./ci_env.sh cargo run --bin pink -- doc rota "<intenção>"` e `./ci_env.sh cargo run --bin pink -- doc mostrar <id>` para a documentação;
2. use `./ci_env.sh cargo run --bin pink -- nav buscar "<conceito>"` e `./ci_env.sh cargo run --bin pink -- nav mostrar <chave>` para o código;
3. leia o `README.md` local do território antes de alterá-lo;
4. consulte [docs/code_map.md](docs/code_map.md) para o mapa por camada;
5. consulte [docs/examples_index.md](docs/examples_index.md) para encontrar um
   exemplo e os testes mais próximos.

O frontend está concentrado em lexer, parser e AST; semântica e layout têm sua
própria camada; IR, CFG, seleção e máquina possuem validações correspondentes;
interpretador, backend, runtime e CLI fecham o pipeline. O mapa de código indica
os arquivos exatos sem duplicá-los aqui.

## Escopo da mudança

Mantenha o menor diff auditável e não misture refatorações não solicitadas.
Distinga o tipo de trabalho:

- **funcional:** altera comportamento da linguagem, compilador ou runtime;
- **documental:** corrige ou reorganiza documentação sem mudar comportamento;
- **operacional:** ajusta o trabalho corrente sem abrir fase funcional, rodada
  documental, hotfix ou inventário futuro por inércia.

Uma mudança funcional real deve trazer evidência em código, testes e
documentação canônica aplicável. Verifique as camadas afetadas do pipeline, os
casos positivos e negativos próximos e, quando couber, um exemplo versionado.
Mudanças da linguagem devem respeitar a trilha ativa e não abrir novas fases
fora da ordem definida no roadmap.

Ao editar documentação, siga [docs/doc_rules.md](docs/doc_rules.md). Preserve
IDs, frontmatter e âncoras `@pinker-doc`. Ao editar código, preserve as regiões
`@pinker-nav:start/end`. Não atualize roadmap, histórico ou handoff apenas por
inércia; atualize fontes canônicas quando o tipo real da mudança assim exigir.

## Build, testes e validação

Use os alvos oficiais durante o desenvolvimento:

```bash
make preflight
make build
make test
make fmt-check
make clippy
make guard
make ci
```

`make ci` é a validação integral. Ela executa, nesta ordem, `preflight`, `build`,
`test`, `fmt-check`, `clippy`, `guard`, `docs-check` e `nav-check`. Antes de
enviar o pull request, execute também `git diff --check` e revise o diff inteiro.
Relate no PR somente validações realmente executadas, com eventuais limitações
do ambiente.

## Trama Pinker e catálogos

A Trama mantém portais para humanos e catálogos para agentes. Os catálogos
`docs/navigation.jsonl` e `src/navigation.jsonl` nunca são editados manualmente.

Quando uma fonte marcada for alterada, sincronize e verifique o catálogo
aplicável. Os alvos do repositório executam, via `pink`, `doc sincronizar`,
`nav sincronizar`, `doc verificar` e `nav verificar`:

```bash
make docs-sync
make nav-sync
make docs-check
make nav-check
```

Pull requests posteriores ao marco #330 usam o único bloco estruturado
`pinker-change`. [O template de pull request](.github/pull_request_template.md)
é a fonte operacional para seus campos, enums e sentinelas. O bloco permite que
a automação valide e projete metadados sem interpretar a narrativa humana; ele
não exige conhecer a arquitetura interna da Trama. Mantenha-o separado da
narrativa e, se a classificação da mudança não estiver clara, peça decisão da
manutenção. Com o número real do PR, importe-o por:

```bash
./ci_env.sh cargo run --bin pink -- doc importar-pr <n> --corpo <arquivo>
```

A política é forward-only: PRs de número menor ou igual a #330 não recebem
backfill. A sincronização cabe a quem prepara a mudança; o CI apenas verifica.

## Commits e pull requests

Faça commits focados no recorte e confira explicitamente os arquivos preparados.
O repositório não estabelece neste guia uma convenção obrigatória de nome de
branch, formato de commit ou título de PR.

No pull request, use o template existente e descreva:

- o que mudou e por que a mudança é necessária;
- decisões técnicas relevantes e limites honestos;
- os comandos de validação executados e seus resultados;
- o bloco `pinker-change`, conforme o template e o marco da Trama.

Um pull request é uma proposta para revisão; este guia não estabelece prazo de
resposta nem garantia de merge.

## Agentes de IA

Agentes devem ler o [AGENTS.md](AGENTS.md) mais próximo do arquivo em trabalho e
as [.github/copilot-instructions.md](.github/copilot-instructions.md)
aplicáveis antes de agir. Esses arquivos são os contratos operacionais para
agentes; este guia não os substitui.

Agentes devem localizar, inspecionar e extrair evidência antes de afirmar estado,
preservar mudanças existentes, evitar ações destrutivas sem autorização e não
declarar testes que não executaram. Merge, release, publicação e outras ações
remotas exigem pedido explícito.

## Limites e licença

Não trate visão como implementação, não transforme inventários em compromisso e
não declare recursos prontos sem código e validação objetiva. Em caso de conflito,
volte às fontes canônicas citadas neste guia e à evidência mergeada.

O repositório é distribuído sob a licença MIT. Consulte [LICENSE](LICENSE) para
o texto e as condições completos.
