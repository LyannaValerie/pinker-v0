# Agent State (operacional)

- Projeto: Pinker v0
- Branch de referência: `main`
- Fonte de verdade: código mergeado no repositório

## Pipeline congelada
semântica -> IR estruturada -> validação IR -> CFG IR -> validação CFG -> seleção -> validação seleção -> Machine -> validação Machine -> pseudo-asm -> validação backend textual.

## Fases concluídas
- Fase 9: disciplina de pilha da Machine
- Fase 10: checagem leve de tipo no topo da pilha
- Fase 11: refinamento de tipos de params/slots na Machine (hotfix de fechamento)
- Fase 12: contexto e mensagens de erro da Machine
- Fase 13: interpretador mínimo com `--run`
- Fase 14: chamadas entre funções no interpretador (`call` e `call_void`)
- Fase 15: globals no interpretador (`load_global`)
- Fase 16: robustez do interpretador e testes negativos de runtime
- Fase 17: recursão coberta por testes dedicados e exemplos CLI
- Fase 18: CI mínima + MSRV
- Fase 19: padronização de mensagens de erro entre IR / CFG / Machine
- Fase 20: mais testes end-to-end com `--run`
- Fase 21a: escrita em globals no interpretador (viabilidade negada no estado atual)
- Fase 21b: stack trace simples de runtime
- Fase 22 documental: doc comments e comentários estruturais em módulos centrais
- Fase 23a: stack trace com contexto ligeiramente melhor + ganchos leves
- Fase 23b: stack trace com contexto ligeiramente melhor + ganchos leves para evolução futura
- Fase 24: melhorar mensagens de erro de runtime além do stack trace
- Fase 25: padronizar e consolidar a renderização final de erros de runtime no CLI
- Fase 26: proteção preventiva contra recursão infinita/limite de profundidade de chamadas
- Fase 27a: adicionar `sempre que`
- Fase 27b: truncamento/resumo de stack trace muito longo
- Fase 28a: adicionar `quebrar` para `sempre que`
- Fase 28b: adicionar `continuar` para `sempre que`
- Fase 28c: melhorar spans/source context em erros de runtime e parser
- Fase 29: consolidar exemplos versionados e cobertura CLI para loops
- Fase 30: consolidar exemplos versionados e cobertura negativa para loops inválidos, e organizar backlog futuro em `docs/future.md`
- Fase 31: adicionar operadores bitwise básicos (`&`, `|`, `^`, `<<`, `>>`)

## Fase atual
- Fase 31 concluída: operadores bitwise básicos adicionados (`&`, `|`, `^`, `<<`, `>>`) com política simples de tipos (`bombom`), mantendo a continuidade histórica e sem expansão fora de escopo.

## Infraestrutura mínima ativa
- Workflow GitHub Actions em `.github/workflows/ci.yml` com `cargo build/check/fmt --check/test`
- MSRV fixada em `rust-toolchain.toml` (`1.78.0`)

## Qualidade diagnóstica (Fase 19)
- IR e CFG IR agora usam contexto padronizado com função/bloco quando aplicável
- Mensagens de incompatibilidade de tipo incluem esperado vs recebido em pontos críticos
- Machine manteve padrão contextual existente (referência de estilo)

## Confiabilidade end-to-end (Fase 20)
- Cobertura adicional de `--run` com exemplos de global+chamada, recursão+global e mutação+if/else
- Cobertura explícita de erro de runtime observado pela CLI (exit non-zero + stderr)

## Viabilidade de globals mutáveis (Fase 21a)
- Semântica atual modela `eterno` como constante não mutável
- Machine atual só possui `LoadGlobal` (sem `StoreGlobal`)
- Interpretador recebe globals por referência imutável (`&HashMap`)

## Stack trace de runtime (Fase 21b)
- Runtime agora anexa `stack trace` textual em erros com nomes de funções ativas
- Ordem do trace: chamada externa -> interna
- Sem label/bloco/locals por frame (adiado para manter escopo pequeno)

## Hotfix operacional (Fase 21b)
- Verificação de duplicação em `tests/interpreter_tests.rs` concluída
- Snapshot atual sem duplicatas ativas para os helpers/testes mapeados
- Stack trace simples de runtime preservado

## Revalidação operacional (Fase 21b)
- Reexecução de `cargo build/check/fmt --check/test` sem falhas
- Reconfirmação do cenário CLI de erro (`examples/run_div_zero_cli.pink`) com stderr enriquecido por stack trace

## Restrições do projeto
- Não expandir linguagem/gramática.
- Não reabrir frontend.
- Não usar LLVM/Cranelift/backend nativo.
- Preservar pipeline e camadas atuais.

## Itens adiados
- Escrita em globals.
- Infraestrutura avançada de runtime (I/O de linguagem, debug runtime, otimizações de execução).
- Inferência global pesada de tipos na Machine.
- Proteção contra recursão infinita/limite de profundidade de chamadas.

## Instrução para novo agente
1. Ler este arquivo primeiro.
2. Ler `docs/handoff_codex.md` e `docs/handoff_auditor.md` antes da rodada.
3. Em caso de conflito, o código mergeado no repositório prevalece.
4. Se `origin/main` não estiver disponível no clone, registrar explicitamente limitação de sincronização.


## Stack trace de runtime (Fase 23a/23b)
- `call_stack` no interpretador migrou de strings ad hoc para `RuntimeFrame`.
- Trace renderizado por helper único (`render_runtime_trace`) para formato estável.
- Contexto adicional atual: label/bloco e instrução atual quando disponível.
- Gancho novo (23b): `current_instr: Option<&'static str>` no frame para evolução barata de contexto.
- Gancho adiado: `future_span` opcional no frame para futura origem por instrução/bloco.


## Mensagens de runtime (Fase 24)
- Erro principal de runtime agora usa prefixo estável por categoria (`[runtime::<tipo>]`).
- Diagnóstico recebeu dica curta para causas comuns (divisão por zero, slot não inicializado, função/global inexistente, aridade inválida).
- Stack trace (`at <função> [bloco] [instr]`) foi mantido sem mudanças estruturais.
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 verificada sem necessidade de correção documental nesta rodada.


## Renderização final de runtime no CLI (Fase 25)
- CLI passou a usar renderização dedicada para `PinkerError::Runtime` via `render_for_cli()`.
- Estrutura textual estabilizada: cabeçalho `Erro Runtime:`, linha `mensagem:`, bloco `stack trace:` (se presente) e linha final `span:`.
- Mensagem principal com categoria estável (`[runtime::<tipo>]`) mantida sem alteração.
- Stack trace existente (`at <função> [bloco] [instr]`) mantido sem alteração funcional.
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 verificada; sem correção documental estrutural nesta rodada.


## Fase 26 — limite preventivo de recursão no runtime
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 verificada e preservada.
- Interpretador passou a aplicar limite interno estável de profundidade (`MAX_CALL_DEPTH = 128`) antes de empilhar novo frame.
- Excesso de profundidade gera erro categorizado `[runtime::limite_recursao_excedido]` com mensagem explícita de limite preventivo do runtime.
- Stack trace por frame (`at <função> [bloco] [instr]`) e renderização consolidada de runtime no CLI foram mantidos sem mudança estrutural.

## Itens adiados (mantidos)
- Configuração externa do limite de recursão.
- Spans ricos por frame (`future_span` segue reservado).
- Debugger/stepping/tracing avançado.


## Fase 27a (estado)
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a verificada.
- Forma de superfície adicionada: `sempre que <condicao> { ... }`.
- Reconhecimento implementado como combinação de palavras-chave `sempre` + `que`.
- Escopo tocado: lexer, parser, AST, semântica, IR estruturada, validação IR, lowering CFG e testes E2E (`--run`).
- Fora de escopo mantido: `enquanto`, `para`, `quebrar`, `continuar`, labels de loop e redesign amplo.


## Fase 27b — truncamento de stack trace longo
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b verificada.
- `render_runtime_trace` passou a truncar traces com mais de `TRACE_TRUNC_THRESHOLD` (10) frames.
- Política: primeiros `TRACE_HEAD` (5) + linha `... N frames omitidos ...` + últimos `TRACE_TAIL` (5).
- Traces curtos (≤ 10 frames) permanecem idênticos ao comportamento anterior.
- Nenhuma mudança de semântica, categorias de erro, frontend ou renderização CLI.

## Itens adiados (mantidos)
- Configuração externa de `MAX_CALL_DEPTH` e do limiar de truncamento.
- Spans ricos por frame (`future_span` segue reservado).
- Debugger/stepping/tracing avançado.


## Fase 28a — adicionar `quebrar` para `sempre que`
- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a verificada.
- `quebrar;` adicionado com escopo mínimo para sair de `sempre que`.
- Parser/AST/semântica/IR/CFG e testes de `--run` atualizados de forma incremental.
- Fora de escopo mantido: `continuar`, labels de loop, alvo de `quebrar`, refactor amplo de fluxo.


## Fase 28b — adicionar `continuar` para `sempre que`

- `continuar;` adicionado com escopo mínimo para avançar para a próxima iteração de `sempre que`.
- Pipeline tocado: lexer/token, parser/AST, semântica, IR estruturada, CFG IR e testes de execução/CLI.
- Fora de escopo mantido: labels de loop, alvo explícito para `continuar`, redesign amplo de fluxo.


## Fase 28c — melhorar spans/source context em erros de runtime e parser

- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c verificada.
- Runtime: span dummy `1:1..1:1` substituído por `localização: indisponível (erro detectado na instrução de máquina)` no CLI.
- Parser/lexer/semântica: novo método `render_for_cli_with_source(source)` extrai e exibe a linha de origem com caret (`^`) alinhado à coluna do erro.
- `main.rs` atualizado para usar `render_for_cli_with_source` após leitura do fonte.
- 3 testes de CLI atualizados; 3 novos testes adicionados (source context parse, source context semântica, localização indisponível runtime).
- Formato geral de runtime (`Erro Runtime:`, `mensagem:`, `stack trace:`) preservado sem mudança.

## Itens adiados (mantidos)
- Spans reais por instrução de máquina (requer propagar spans do AST até Machine).
- `future_span` em `RuntimeFrame` segue reservado mas não preenchido.
- Debugger/stepping/tracing avançado.


## Fase 29 — consolidar exemplos versionados e cobertura CLI de loops

- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 verificada e preservada.
- Testes CLI de loop foram consolidados para usar exemplos versionados:
  - `cli_run_sempre_que_funciona` agora usa `examples/run_sempre_que.pink`;
  - `cli_run_quebrar_funciona` agora usa `examples/run_quebrar.pink`;
  - `cli_run_continuar_funciona` agora usa `examples/run_continuar.pink`.
- Novos exemplos mínimos adicionados: `run_quebrar.pink` e `run_continuar.pink`.
- Semântica de `sempre que`, `quebrar` e `continuar` mantida sem alteração.

## Itens adiados (mantidos)
- Nenhum novo construto de linguagem (`para`, labels de loop, etc.).
- Nenhum redesign de runtime/stack trace nesta fase.


## Fase 30 — cobertura negativa de loops e backlog futuro

- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 verificada e preservada.
- Cobertura negativa reprodutível consolidada com exemplos versionados:
  - `examples/check_quebrar_fora_loop.pink`
  - `examples/check_continuar_fora_loop.pink`
- Testes CLI com `--check` adicionados para garantir erro semântico estável por substring:
  - `cli_check_quebrar_fora_de_loop_falha_com_exemplo_versionado`
  - `cli_check_continuar_fora_de_loop_falha_com_exemplo_versionado`
- `docs/handoff_opus.md` descontinuado e redirecionado para `docs/future.md`.
- `docs/future.md` criado para concentrar backlog de longo prazo por camadas (0..8), mantendo fora do escopo imediato.

## Itens adiados (mantidos)
- Sem novas keywords/construtos além do já existente (`sempre que`, `quebrar`, `continuar`).
- Sem redesign de runtime/stack trace.
- Sem expansão de gramática/arquitetura fora da fase.


## Fase 31 — operadores bitwise básicos

- Continuidade histórica 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 → 31 verificada e preservada.
- Operadores adicionados: `&`, `|`, `^`, `<<`, `>>`.
- Pipeline tocado de forma incremental: lexer/token, parser/AST, semântica, IR estruturada, CFG IR, seleção, Machine e interpretador.
- Política de tipos adotada: operações bitwise e shifts aceitas apenas para `bombom`; uso em `logica` segue inválido.
- Cobertura adicionada em testes de lexer/parser/semântica/IR/CFG/selected/machine/interpreter + exemplo `examples/run_bitwise_basico.pink`.
- Fora de escopo mantido: operadores compostos, `&&`/`||`, novos tipos inteiros e redesign amplo.
