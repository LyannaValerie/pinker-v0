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

- Fase 15 — concluída
  - suporte a leitura de globals no interpretador (`load_global`) com mapa de globals por execução
  - avaliação mínima de globals literais inteiras/lógicas para `RuntimeValue`
  - erro explícito para global inexistente e para valor global não suportado em runtime

- Fase 16 — concluída
  - 6 testes negativos de runtime via MachineProgram manual: divisão por zero, slot não inicializado, aridade inválida, call/call_void mismatch, valor global não suportado
  - 8 testes end-to-end via run_code: Not, Div, CmpEq, CmpNe, CmpGe, CmpGt, CmpLe, reassignment de variável mutável
  - 1 teste CLI: exit code não-zero e stderr não vazio em erro de runtime
  - pequeno endurecimento: mensagens de erro em call_function incluem o nome da função ([fn_name])

- Fase 17 — concluída
  - cobertura dedicada de recursão no interpretador sem alteração estrutural
  - 4 testes novos em `interpreter_tests`: fatorial, fibonacci, recursão linear e recursão mútua
  - exemplos CLI adicionados: `examples/run_fatorial.pink` e `examples/run_fibonacci.pink`
  - validação end-to-end com `cargo run -- --run` para ambos os exemplos


- Fase 18 — concluída
  - CI mínima adicionada em `.github/workflows/ci.yml`
  - checks de CI: `cargo build --locked`, `cargo check --locked`, `cargo fmt --check`, `cargo test --locked`
  - política de MSRV definida como Rust `1.78.0` em `rust-toolchain.toml` e documentada no README


- Fase 19 — concluída
  - padronização de mensagens de erro entre validadores IR, CFG IR e Machine
  - IR: contexto de função/bloco/instrução e detalhes esperado vs recebido em incompatibilidades de tipo
  - CFG IR: contexto de função/bloco e enriquecimento pontual com detalhe técnico de instrução
  - Machine mantida sem alteração estrutural, com teste garantindo formato contextual estável
