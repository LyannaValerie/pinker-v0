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

## Fase atual
- Fase 23b concluída: stack trace de runtime mantém frames estruturados e adiciona contexto de instrução por frame
  (`at <função> [bloco: <label>] [instr: <op>]`) com ganchos leves (`future_span`) preservados.

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
