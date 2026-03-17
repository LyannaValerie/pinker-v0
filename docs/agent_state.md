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

## Fase atual
- Fase 14 concluída (execução com chamadas entre funções na Machine interpretada).

## Restrições do projeto
- Não expandir linguagem/gramática.
- Não reabrir frontend.
- Não usar LLVM/Cranelift/backend nativo.
- Preservar pipeline e camadas atuais.

## Itens adiados
- Execução com globals.
- Infraestrutura avançada de runtime (I/O de linguagem, debug runtime, otimizações de execução).
- Inferência global pesada de tipos na Machine.

## Instrução para novo agente
1. Ler este arquivo primeiro.
2. Codex: ler `docs/handoff_opus.md` antes de iniciar rodada.
3. Opus: após pull do `main`, ler `docs/handoff_codex.md`.
4. Em caso de conflito, o código mergeado no repositório prevalece.
