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

## Fase atual
- Fase 19 concluída (mensagens de validação padronizadas entre IR/CFG/Machine, com contexto técnico consistente).

## Infraestrutura mínima ativa
- Workflow GitHub Actions em `.github/workflows/ci.yml` com `cargo build/check/fmt --check/test`
- MSRV fixada em `rust-toolchain.toml` (`1.78.0`)

## Qualidade diagnóstica (Fase 19)
- IR e CFG IR agora usam contexto padronizado com função/bloco quando aplicável
- Mensagens de incompatibilidade de tipo incluem esperado vs recebido em pontos críticos
- Machine manteve padrão contextual existente (referência de estilo)

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
