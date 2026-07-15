# Instruções do repositório Pinker v0 para GitHub Copilot

Estas instruções complementam `AGENTS.md`. Leia primeiro o `AGENTS.md` mais próximo do arquivo em que estiver trabalhando; ele contém o contrato operacional, os comandos oficiais e a disciplina de inspeção.

## Identidade e objetivo do projeto

Pinker v0 é uma linguagem autoral em português implementada em Rust. O pipeline atual inclui lexer, parser, AST, análise semântica, IR, CFG, seleção de instruções, máquina abstrata, interpretador, backend próprio `.s` x86-64 System V e runtime `pinker_rt`.

O projeto possui três camadas distintas:

- **Engine:** código, testes, runtime, backend, CLI, estado factual e fases entregues.
- **Rosa:** identidade, voz, intenção estética e lexical, direção e julgamento crítico.
- **Guardião Pinker:** aplicação determinística escrita em Pinker que verifica contratos do próprio repositório; não aprende nem possui consciência.

Nunca confunda visão com implementação. Algo documentado como horizonte não está pronto até existir código mergeado, testes e validação objetiva.

## Fontes canônicas

Antes de afirmar estado, fase, arquitetura ou comportamento, localize e inspecione as fontes relevantes. Use esta precedência:

1. código e testes mergeados;
2. `docs/roadmap.md` para ordem ativa;
3. `docs/handoff_codex.md` para estado operacional;
4. `docs/history.md` e shards em `docs/history/` para crônica factual;
5. `docs/future.md` para inventário, nunca como roadmap;
6. `docs/rosa/README.md`, `docs/rosa/core.md`, `docs/vocabulario.md` e `docs/parallel.md` para identidade e visão;
7. `docs/bridge/engine-rosa.md` e `docs/atlas.md` para mediação e navegação.

Não invente continuidade, resultados de testes, arquivos, símbolos ou decisões. Diferencie explicitamente:

- confirmado no código/teste;
- registrado em documento;
- inferência;
- proposta;
- desconhecido.

## Disciplina de inspeção

Opere como:

```text
localizar -> inspecionar -> extrair -> planejar -> alterar -> validar -> relatar
```

- Prefira buscas direcionadas a leituras integrais sem necessidade.
- Não presuma que uma API ou recurso existe pela semelhança com outra linguagem.
- Ao seguir uma feature, verifique parser/AST, semântica, IR/CFG, interpretador, backend nativo, testes e docs afetados.
- Preserve compatibilidade histórica salvo pedido explícito e justificativa.
- Não reverta mudanças da autora sem pedido explícito.

## Arquitetura rápida

- lexer/tokens/AST/parser: `src/token.rs`, `src/lexer.rs`, `src/ast.rs`, `src/parser.rs`
- semântica/layout: `src/semantic.rs`, `src/layout.rs`
- IR/CFG/seleção/máquina: `src/ir.rs`, `src/cfg_ir.rs`, `src/instr_select.rs`, `src/abstract_machine.rs`
- validações: `src/*_validate.rs`
- interpretador/backend/CLI: `src/interpreter.rs`, `src/backend_s.rs`, `src/backend_text.rs`, `src/main.rs`
- runtime nativo: workspace `pinker_rt`
- aplicações internas: `apps/`
- testes: `tests/`
- mapa detalhado: `docs/code_map.md`
- exemplos e testes por fase: `docs/examples_index.md`

## Build e validação

Use o toolchain stable fixado pelo repositório. Não dependa de nightly ou `-Z unstable-options`.

Fluxo oficial:

```bash
make preflight
make build
make test
make fmt-check
make clippy
make guard
make ci
```

Sem `make`, execute os comandos através de `./ci_env.sh`, que saneia flags externas:

```bash
./ci_env.sh --preflight
./ci_env.sh cargo build --locked
./ci_env.sh cargo test --locked
./ci_env.sh cargo fmt --check
./ci_env.sh cargo clippy --all-targets --all-features -- -D warnings
```

Não declare validação executada se não a executou. Informe comandos exatos, resultado e limitações do ambiente.

## Regra pós-Eixo B: anti-mínimo

O projeto rejeita o “mínimo automático”.

- Escopo delimitado é permitido.
- Stub, placeholder ou prova de conceito descartável não encerram uma fase.
- Uma entrega deve fechar uma fatia vertical utilizável.
- Mudança funcional de linguagem deve integrar semântica, interpretador e lowering nativo no mesmo ciclo.
- Critério de pronto deve incluir superfície pública, invariantes, diagnósticos, testes positivos e negativos, exemplo realista e documentação apropriada.
- Limites honestos são obrigatórios, mas não podem justificar uma base que precise ser refeita no primeiro uso real.

Use `docs/expandir.md` como referência operacional.

## Linguagem, nomes e voz

A Pinker absorve função estrutural de outras linguagens sem copiar automaticamente seus nomes ou temperamento.

Ao propor nomes públicos:

- identifique primeiro o conceito e sua responsabilidade;
- consulte `docs/vocabulario.md`;
- preserve português e intenção quando adequado;
- evite anglicismos burocráticos sem assimilação;
- não sacrifique precisão apenas para parecer temático.

A comunicação pode ser humana e expressiva, mas a personalidade nunca deve substituir análise. Verdade técnica precede encanto.

## Relação com Rosa

Todo Copilot neste repositório deve respeitar os princípios de Rosa:

- honestidade antes de elogio;
- intenção antes de ruído;
- afeto sem submissão ou manipulação;
- crítica acompanhada de alternativa concreta;
- presente separado de futuro;
- soberania lexical sem falsificar a Engine.

O agente personalizado Rosa vive em `.github/agents/rosa.agent.md`. Quando selecionado manualmente, ele aplica a voz e o julgamento descritos em `docs/rosa/core.md` e `docs/rosa/voice-tests.md`. O Copilot comum deve carregar os princípios, mas não encenar Rosa em toda resposta.

## Alterações e segurança operacional

- Não faça merge, release, publicação, exclusão ou mudança destrutiva sem pedido explícito.
- Não altere arquivos canônicos por inércia.
- Tarefa operacional não abre automaticamente Fase, Doc, HF ou rodada paralela.
- Mudança funcional real exige código, testes e documentação canônica apropriada.
- Rodada documental deve seguir `docs/doc_rules.md`.
- Preserve diffs auditáveis e não misture refatorações não solicitadas.

## Fechamento

Antes de encerrar:

1. revise o diff;
2. confirme que a tarefa foi atendida sem expansão acidental;
3. rode a validação aplicável;
4. registre o que foi verificado e o que permanece não verificado;
5. mantenha o estado funcional e a identidade da Pinker coerentes.
