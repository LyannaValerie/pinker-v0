# Linha do tempo de fases

- Fase 9 — concluída
  - disciplina de pilha, underflow, consistência entre predecessores, slots/temporários, aridade

- Fase 10 — concluída
  - checagem leve de tipo no topo da pilha (`br_true`, `ret`, unárias/binárias quando inferível)

- Fase 11 — concluída (revalidada nesta rodada)
  - refinamento de tipos de params/slots no checker da Machine
  - regressões tipadas para `call` e `call_void`
  - `cargo build`, `cargo check`, `cargo fmt --check` e `cargo test` passando


- Fase 12 — concluída
  - enriquecimento de contexto/mensagens na validação da Machine (função, bloco, instrução/terminador, esperado vs recebido)
  - cobertura de testes para underflow, tipos incompatíveis, `ret`, `br_true`, slots, `call` e `call_void`

- Fase 13 — concluída
  - interpretador mínimo da Machine com `--run` (execução de `principal` com frame local de slots/pilha e fluxo por labels)
  - suporte inicial a push/load/store, unárias/binárias, comparações numéricas, `jmp`, `br_true`, `ret` e `ret_void`
  - falhas explícitas para `call`, `call_void`, globals e execução multi-função

- Fase 14 — concluída
  - suporte a chamadas entre funções no interpretador (`call` e `call_void`) com frame por função
  - ordem de argumentos preservada no runtime (desempilha + reverse)
  - erro explícito para função inexistente e para `call_void` recebendo retorno
