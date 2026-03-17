# Handoff Codex (executor)

## Rodada atual
- Avaliação da **FASE 21a — escrita em globals no interpretador**.

## Objetivo
- Verificar viabilidade real de escrita/mutação de globals no estado atual da linguagem + pipeline.
- Implementar apenas se já existisse caminho coerente e pequeno no código atual.

## Estado real encontrado
- Workspace local usado como fonte de verdade.
- Base inicial saudável: `cargo build` e `cargo test` passando.

## Diagnóstico de viabilidade (factual)
A fase **não é viável tecnicamente** no estado atual sem abrir escopo fora do permitido.

### Evidências no código real
1. **Semântica trata globais como constantes imutáveis**
   - `resolve_var` mapeia constantes globais com `is_mut: false`.
   - Atribuição em `Stmt::Assign` exige `is_mut == true` e rejeita reatribuição de símbolo não mutável.

2. **Machine não possui instrução de escrita em global**
   - `MachineInstr` expõe `LoadGlobal`, mas não existe `StoreGlobal`.

3. **Interpretador recebe globals como mapa somente leitura**
   - `run_program` cria `globals` e passa `&HashMap<String, RuntimeValue>` para execução.
   - `exec_instr` implementa `LoadGlobal`, mas não há ramo de escrita em global.

4. **Lowering atual não produz operação de escrita em global**
   - Caminho de lowering para Machine usa `StoreSlot` para slots locais/temporários; não há lowering para store global.

## Decisão aplicada
- Não foi implementada escrita em globals.
- Não foi criado exemplo/feature parcial inconsistente.
- Rodada tratada como auditoria curta de impossibilidade atual, conforme instrução.

## Bloqueio
Para viabilizar Fase 21a de fato, seria necessário (fora do escopo mínimo desta rodada):
- decisão de linguagem/semântica para mutabilidade de `eterno` (ou novo construto)
- suporte em IR/CFG/seleção/Machine para operação de escrita global
- validação estrutural e de tipos para novo opcode
- atualização do interpretador para estado global mutável compartilhado entre chamadas

## Arquivos alterados
- `docs/handoff_codex.md`
- `docs/agent_state.md`
- `docs/phases.md`

## Comandos executados
- Inicial:
  - `cargo build`
  - `cargo test`
- Final:
  - `cargo build`
  - `cargo check`
  - `cargo fmt --check`
  - `cargo test`

## Próximos passos sugeridos
- Decidir explicitamente a política de mutabilidade de globals na linguagem (se entra ou não).
- Se aprovado, abrir fase dedicada de pipeline (semântica + IR/CFG/Machine + runtime), em vez de patch isolado no interpretador.
