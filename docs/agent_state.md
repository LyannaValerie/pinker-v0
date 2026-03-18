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

## Fase atual
- Fase 27a concluída: suporte mínimo a `sempre que <condicao> { ... }` no pipeline da linguagem, mantendo as fases 21a → 26 preservadas.

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
