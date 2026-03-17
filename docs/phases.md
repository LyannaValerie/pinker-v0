# Linha do tempo de fases

- Fase 9 — concluída
  - disciplina de pilha, underflow, consistência entre predecessores, slots/temporários, aridade

- Fase 10 — concluída
  - checagem leve de tipo no topo da pilha (`br_true`, `ret`, unárias/binárias quando inferível)

- Fase 11 — concluída (revalidada nesta rodada)
  - refinamento de tipos de params/slots no checker da Machine
  - regressões tipadas para `call` e `call_void`
  - `cargo build`, `cargo check`, `cargo fmt --check` e `cargo test` passando
