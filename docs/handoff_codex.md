# Handoff Codex (executor)

## Rodada atual
- Implementação da **Fase 12** na validação da Machine: enriquecimento de contexto e mensagens de erro, sem alterar semântica/pipeline.

## Objetivo
- Melhorar diagnósticos de erro da camada Machine com contexto de função/bloco e detalhe da instrução/terminador relevante.
- Cobrir cenários críticos (underflow, incompatibilidade de tipo, `ret`, `br_true`, slots, `call` e `call_void`) mantendo validações existentes.

## Arquivos alterados
- `src/abstract_machine_validate.rs`
- `tests/abstract_machine_stack_tests.rs`
- `docs/handoff_codex.md`
- `docs/phases.md`

## Diagnóstico real encontrado
- O projeto permanece no escopo congelado de frontend + pipeline textual.
- A validação da Machine já fazia checagens estruturais e disciplina de pilha (Fases 9–11), mas mensagens ainda eram genéricas em vários pontos.

## Decisão técnica aplicada
- Mantida a arquitetura atual (`PinkerError::AbstractMachineValidation`), sem redesign da hierarquia.
- Introduzido detalhe opcional em erros da validação (`err_ctx_with_detail`) para anexar contexto local da instrução/terminador.
- `ensure_compatible` agora informa `esperado` vs `recebido` quando há tipo incompatível.

## Testes/comandos executados
- `cargo build`
- `cargo check`
- `cargo fmt --check`
- `cargo test`

## Resultado real
- Todos os comandos passaram.
- Regressão total estável.
- Mensagens da Machine agora incluem contexto adicional auditável para os casos-alvo da Fase 12.

## Limitações
- Tipagem da Machine continua leve/local (sem inferência global pesada).
- Sem interpretador/execução da Machine (adiado por escopo).

## Pendências
- Nenhuma pendência da Fase 12 nesta rodada.
