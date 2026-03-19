# Agent State (operacional)

- Projeto: Pinker v0
- Branch de referência: `main`
- Fonte de verdade: código mergeado no repositório

## Diretriz consolidada de execução (rodada documental atual)
- Não abrir trilha paralela.
- Seguir a ordem consolidada em `docs/roadmap.md`.
- Não pular para backend nativo antes da base mínima de tipos/modelagem.
- Não antecipar módulos/strings/I/O à frente da trilha de kernel, salvo rodada documental explícita.
- Assembly textual `.s` é a estratégia inicial preferível para backend.
- Cada fase funcional deve permanecer pequena, auditável e com dependência real.
- `docs/roadmap.md` tem precedência de execução sobre `docs/future.md`.
- Usar checklist final para confirmar fases anteriores, evitando repetir torre histórica no corpo das respostas.

## Convenção de fases e rodadas (ativa)
- Fase numerada (`Fase N`) = entrega funcional/estrutural real.
- Rodada documental = ajuste documental/estratégico sem feature funcional.
- Rodada documental não recebe número de fase.

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
- Rodada documental: viabilidade de escrita em globals no interpretador (negada no estado atual)
- Fase 21: stack trace simples de runtime
- Rodada documental: doc comments e comentários estruturais em módulos centrais
- Fase 22: stack trace com contexto ligeiramente melhor + ganchos leves
- Fase 23: stack trace com contexto ligeiramente melhor + ganchos leves para evolução futura
- Fase 24: melhorar mensagens de erro de runtime além do stack trace
- Fase 25: padronizar e consolidar a renderização final de erros de runtime no CLI
- Fase 26: proteção preventiva contra recursão infinita/limite de profundidade de chamadas
- Fase 27: adicionar `sempre que`
- Fase 28: truncamento/resumo de stack trace muito longo
- Fase 29: adicionar `quebrar` para `sempre que`
- Fase 30: adicionar `continuar` para `sempre que`
- Fase 31: melhorar spans/source context em erros de runtime e parser
- Fase 32: consolidar exemplos versionados e cobertura CLI para loops
- Fase 33: consolidar exemplos versionados e cobertura negativa para loops inválidos, e organizar backlog futuro em `docs/future.md`
- Fase 34: adicionar operadores bitwise básicos (`&`, `|`, `^`, `<<`, `>>`)
- Fase 35: robustez de lowering CFG para `talvez/senao` com fall-through em ambos os ramos
- Fase 36: adicionar operadores lógicos `&&` e `||` com short-circuit
- Fase 37: adicionar licença do projeto e documentar seu uso básico
- Fase 38: humanizar a renderização de `--machine` sem alterar a Machine
- Fase 39: humanizar instruções individuais de `--machine` com comentários curtos
- Fase 40: contextualizar os comentários de `--machine` por alvo/slot sem alterar semântica
- Fase 41: tornar os comentários de `--machine` sensíveis ao papel do fluxo
- Fase 42: operador `%` nativo (primeira fase funcional do Bloco 1), com integração no pipeline completo e cobertura de runtime/CLI
- Fase 43: inteiros unsigned fixos (`u8`, `u16`, `u32`, `u64`) com validação estrita e integração no pipeline
- Fase 44: inteiros signed fixos (`i8`, `i16`, `i32`, `i64`) com validação estrita e integração no pipeline
- Fase 45: aliases de tipo com keyword `apelido`, resolução para tipo subjacente e validações de alias inexistente/duplicado/recursivo
- Rodada documental: consolidação da trilha única "voltar aos trilhos" em `docs/roadmap.md` (sem número de fase)

## Fase atual
- Fase 47 concluída: structs com keyword `ninho` como tipo nomeado composto (declaração + registro semântico + uso tipado em assinaturas/aliases), com validação de campo duplicado, tipo inexistente e recursão direta; integração mínima no IR como categoria `struct` sem acesso a campo/literal/layout.
- Bloco 1 encerrado com a Fase 47; trilha ativa segue no Bloco 2.
- Fase 48 concluída: ponteiros como categoria de tipo com `seta<tipo>` (frontend + semântica + IR), sem semântica operacional de memória.
- Fase 48-H1 concluída: rodada extraordinária de hotfixes de corretude e manutenção (HF-1 a HF-17).
- Fase 49 concluída: acesso a campo (`obj.campo`) e indexação (`arr[idx]`) com escopo mínimo de leitura.
- Fase 50 concluída: casts controlados com `virar` (escopo explícito e conservador), com suporte frontend/semântica/IR para inteiro->inteiro e sem lowering operacional em CFG/Machine/runtime.

## Infraestrutura mínima ativa
- Workflow GitHub Actions em `.github/workflows/ci.yml` com `cargo build/check/fmt --check/clippy/test/doc`
- MSRV fixada em `rust-toolchain.toml` (`1.78.0`) com componentes `rustfmt` e `clippy`
- CI alinhada: usa `dtolnay/rust-toolchain@master` com `toolchain: "1.78.0"` (não mais `@stable`)

## Qualidade diagnóstica (Fase 19)
- IR e CFG IR agora usam contexto padronizado com função/bloco quando aplicável
- Mensagens de incompatibilidade de tipo incluem esperado vs recebido em pontos críticos
- Machine manteve padrão contextual existente (referência de estilo)

## Confiabilidade end-to-end (Fase 20)
- Cobertura adicional de `--run` com exemplos de global+chamada, recursão+global e mutação+if/else
- Cobertura explícita de erro de runtime observado pela CLI (exit non-zero + stderr)

## Rodada documental — viabilidade de globals mutáveis
- Semântica atual modela `eterno` como constante não mutável
- Machine atual só possui `LoadGlobal` (sem `StoreGlobal`)
- Interpretador recebe globals por referência imutável (`&HashMap`)

## Stack trace de runtime (Fase 21)
- Runtime agora anexa `stack trace` textual em erros com nomes de funções ativas
- Ordem do trace: chamada externa -> interna
- Sem label/bloco/locals por frame (adiado para manter escopo pequeno)

## Rodada documental — hotfix operacional pós-Fase 21
- Verificação de duplicação em `tests/interpreter_tests.rs` concluída
- Snapshot atual sem duplicatas ativas para os helpers/testes mapeados
- Stack trace simples de runtime preservado

## Rodada documental — revalidação operacional pós-Fase 21
- Reexecução de `cargo build/check/fmt --check/test` sem falhas
- Reconfirmação do cenário CLI de erro (`examples/run_div_zero_cli.pink`) com stderr enriquecido por stack trace

## Restrições do projeto
- Manter fases pequenas, auditáveis e na ordem de `docs/roadmap.md`.
- Evitar refactor amplo fora do escopo da fase ativa.
- Não usar LLVM/Cranelift/backend nativo.
- Preservar pipeline e camadas atuais.

## Itens adiados
- Escrita em globals.
- Infraestrutura avançada de runtime (I/O de linguagem, debug runtime, otimizações de execução).
- Inferência global pesada de tipos na Machine.
- Proteção contra recursão infinita/limite de profundidade de chamadas.
- Dereferência e operações reais de ponteiro (load/store indireto, aritmética de ponteiro, campo/indexação via ponteiro, casts, `sizeof`/alinhamento, `volatile`).

## Instrução para novo agente
1. Ler este arquivo primeiro.
2. Ler `docs/handoff_codex.md` antes da rodada. (`docs/handoff_auditor.md` está formalmente abandonado — não ler como ativo.)
3. Em caso de conflito, o código mergeado no repositório prevalece.
4. Se `origin/main` não estiver disponível no clone, registrar explicitamente limitação de sincronização.


## Stack trace de runtime (Fases 22/23)
- `call_stack` no interpretador migrou de strings ad hoc para `RuntimeFrame`.
- Trace renderizado por helper único (`render_runtime_trace`) para formato estável.
- Contexto adicional atual: label/bloco e instrução atual quando disponível.
- Gancho novo (Fase 23): `current_instr: Option<&'static str>` no frame para evolução barata de contexto.
- Gancho adiado: `future_span` opcional no frame para futura origem por instrução/bloco.


## Mensagens de runtime (Fase 24)
- Erro principal de runtime agora usa prefixo estável por categoria (`[runtime::<tipo>]`).
- Diagnóstico recebeu dica curta para causas comuns (divisão por zero, slot não inicializado, função/global inexistente, aridade inválida).
- Stack trace (`at <função> [bloco] [instr]`) foi mantido sem mudanças estruturais.
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.


## Renderização final de runtime no CLI (Fase 25)
- CLI passou a usar renderização dedicada para `PinkerError::Runtime` via `render_for_cli()`.
- Estrutura textual estabilizada: cabeçalho `Erro Runtime:`, linha `mensagem:`, bloco `stack trace:` (se presente) e linha final `span:`.
- Mensagem principal com categoria estável (`[runtime::<tipo>]`) mantida sem alteração.
- Stack trace existente (`at <função> [bloco] [instr]`) mantido sem alteração funcional.
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.


## Fase 26 — limite preventivo de recursão no runtime
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Interpretador passou a aplicar limite interno estável de profundidade (`MAX_CALL_DEPTH = 128`) antes de empilhar novo frame.
- Excesso de profundidade gera erro categorizado `[runtime::limite_recursao_excedido]` com mensagem explícita de limite preventivo do runtime.
- Stack trace por frame (`at <função> [bloco] [instr]`) e renderização consolidada de runtime no CLI foram mantidos sem mudança estrutural.

## Itens adiados (mantidos)
- Configuração externa do limite de recursão.
- Spans ricos por frame (`future_span` segue reservado).
- Debugger/stepping/tracing avançado.


## Fase 27 (estado)
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Forma de superfície adicionada: `sempre que <condicao> { ... }`.
- Reconhecimento implementado como combinação de palavras-chave `sempre` + `que`.
- Escopo tocado: lexer, parser, AST, semântica, IR estruturada, validação IR, lowering CFG e testes E2E (`--run`).
- Fora de escopo mantido: `enquanto`, `para`, `quebrar`, `continuar`, labels de loop e redesign amplo.


## Fase 28 — truncamento de stack trace longo
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `render_runtime_trace` passou a truncar traces com mais de `TRACE_TRUNC_THRESHOLD` (10) frames.
- Política: primeiros `TRACE_HEAD` (5) + linha `... N frames omitidos ...` + últimos `TRACE_TAIL` (5).
- Traces curtos (≤ 10 frames) permanecem idênticos ao comportamento anterior.
- Nenhuma mudança de semântica, categorias de erro, frontend ou renderização CLI.

## Itens adiados (mantidos)
- Configuração externa de `MAX_CALL_DEPTH` e do limiar de truncamento.
- Spans ricos por frame (`future_span` segue reservado).
- Debugger/stepping/tracing avançado.


## Fase 29 — adicionar `quebrar` para `sempre que`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `quebrar;` adicionado com escopo mínimo para sair de `sempre que`.
- Parser/AST/semântica/IR/CFG e testes de `--run` atualizados de forma incremental.
- Fora de escopo mantido: `continuar`, labels de loop, alvo de `quebrar`, refactor amplo de fluxo.


## Fase 30 — adicionar `continuar` para `sempre que`

- `continuar;` adicionado com escopo mínimo para avançar para a próxima iteração de `sempre que`.
- Pipeline tocado: lexer/token, parser/AST, semântica, IR estruturada, CFG IR e testes de execução/CLI.
- Fora de escopo mantido: labels de loop, alvo explícito para `continuar`, redesign amplo de fluxo.


## Fase 31 — melhorar spans/source context em erros de runtime e parser

- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Runtime: span dummy `1:1..1:1` substituído por `localização: indisponível (erro detectado na instrução de máquina)` no CLI.
- Parser/lexer/semântica: novo método `render_for_cli_with_source(source)` extrai e exibe a linha de origem com caret (`^`) alinhado à coluna do erro.
- `main.rs` atualizado para usar `render_for_cli_with_source` após leitura do fonte.
- 3 testes de CLI atualizados; 3 novos testes adicionados (source context parse, source context semântica, localização indisponível runtime).
- Formato geral de runtime (`Erro Runtime:`, `mensagem:`, `stack trace:`) preservado sem mudança.

## Itens adiados (mantidos)
- Spans reais por instrução de máquina (requer propagar spans do AST até Machine).
- `future_span` em `RuntimeFrame` segue reservado mas não preenchido.
- Debugger/stepping/tracing avançado.


## Fase 32 — consolidar exemplos versionados e cobertura CLI de loops

- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Testes CLI de loop foram consolidados para usar exemplos versionados:
  - `cli_run_sempre_que_funciona` agora usa `examples/run_sempre_que.pink`;
  - `cli_run_quebrar_funciona` agora usa `examples/run_quebrar.pink`;
  - `cli_run_continuar_funciona` agora usa `examples/run_continuar.pink`.
- Novos exemplos mínimos adicionados: `run_quebrar.pink` e `run_continuar.pink`.
- Semântica de `sempre que`, `quebrar` e `continuar` mantida sem alteração.

## Itens adiados (mantidos)
- Nenhum novo construto de linguagem (`para`, labels de loop, etc.).
- Nenhum redesign de runtime/stack trace nesta fase.


## Fase 33 — cobertura negativa de loops e backlog futuro

- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
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


## Fase 34 — operadores bitwise básicos

- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Operadores adicionados: `&`, `|`, `^`, `<<`, `>>`.
- Pipeline tocado de forma incremental: lexer/token, parser/AST, semântica, IR estruturada, CFG IR, seleção, Machine e interpretador.
- Política de tipos adotada: operações bitwise e shifts aceitas apenas para `bombom`; uso em `logica` segue inválido.
- Cobertura adicionada em testes de lexer/parser/semântica/IR/CFG/selected/machine/interpreter + exemplo `examples/run_bitwise_basico.pink`.
- Fora de escopo mantido: operadores compostos, `&&`/`||`, novos tipos inteiros e redesign amplo.


## Fase 35 — robustez do lowering CFG para `talvez/senao` com fall-through em ambos os ramos

- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Cobertura estrutural adicionada no CFG IR para `if-else` com queda em ambos os ramos, validando presença de branch, jumps para `join` e retorno final sem panic.
- Cobertura CLI reforçada para cenário representativo (`examples/algoritmo_complexo.pink`) mantendo resultado estável.
- Sem alteração de semântica/gramática e sem refactor amplo de lowering.

## Itens adiados (mantidos)
- Refactor estrutural maior do lowerer de CFG.
- Expansões de linguagem fora da Fase 35.


## Fase 36 — operadores lógicos `&&` e `||` com short-circuit
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Frontend atualizado de forma mínima: `AmpAmp`/`PipePipe` em token/lexer, parse de `&&`/`||` e nós `LogicalAnd`/`LogicalOr`.
- Política de tipos adotada: operadores lógicos funcionam somente com `logica` e retornam `logica`.
- Short-circuit garantido no lowering CFG via blocos `logic_rhs_*`, `logic_short_*` e `logic_join_*`; o RHS não é avaliado quando o LHS já determina o resultado.
- Cobertura adicionada em lexer/parser/semântica/IR/CFG/interpreter e em execução CLI `--run` com exemplos dedicados.

## Itens adiados (mantidos após Fase 36)
- Truthiness implícito.
- Coerções implícitas/overloads de operadores.
- Operadores compostos lógicos relacionados.


## Fase 38 — humanizar a renderização de `--machine`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- Renderização de `--machine` (função `render_program` em `abstract_machine.rs`) tornou-se substancialmente mais legível.
- Nomes de params e locals exibidos limpos (`x, y` em vez de `%x#0, %y#0`).
- Temporários internos (`%t0`, `%t1`, …) listados em linha `temps` no cabeçalho da função — visualmente separados de variáveis do usuário.
- Blocos recebem anotação de papel como comentário inline (ex: `entry:  ; entrada da função`, `then_0:  ; ramo 'verdadeiro' (talvez)`).
- Machine, interpretador, semântica e qualquer outra camada funcional: NÃO alterados.
- `--selected` NÃO alterado (deliberadamente fora do escopo).

## Itens adiados (mantidos após Fase 38)
- Spans reais por instrução de máquina (requer propagar spans do AST até MachineInstr).
- Modo alternativo de renderização ou `--machine-legivel` — desnecessário dado o resultado desta fase.

## Fase 39 — humanizar instruções individuais de `--machine`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `render_instr` passou a anexar comentários curtos de intenção por operação (carregar, gravar, chamar, operar/comparar).
- `render_term` passou a explicar fluxo de controle por terminador (`br_true`, `jmp`, `ret`, `ret_void`).
- A instrução técnica original foi mantida sem abreviação ou ocultação; semântica da Machine inalterada.
- Cobertura de testes atualizada para novo formato e reforçada com checks de substring estável para `call`, `br_true`, `jmp` e `ret`.
- `--selected`, `--cfg-ir`, `--pseudo-asm`, `--run`, parser, lowering CFG e interpretador permaneceram sem mudanças.


## Fase 40 — contextualizar os comentários de `--machine`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `render_instr` diferencia `store_slot` em temporário (`%tN`) vs variável local do usuário.
- `render_instr` enriquece `call`/`call_void` com nome da função e aridade de forma natural.
- `render_term` contextualiza `br_true` (if/loop/curto-circuito) e `jmp` por alvo conhecido.
- `ret` e `ret_void` receberam comentários mais claros (`retorna o valor atual da pilha` / `encerra a função sem retorno`).
- Sem mudanças em semântica, parser, lowering CFG, interpretador, `--selected`, `--cfg-ir`, `--pseudo-asm` e `--run`.

## Fase 41 — comentários sensíveis ao papel do fluxo em `--machine`
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `render_term` passou a receber o label do bloco atual para contextualizar comentários por papel de fluxo.
- `br_true` foi refinado para casos de `if`, `sempre que` e curto-circuito (`logic_short_*`/`logic_rhs_*`) com texto mais específico.
- `jmp` ganhou comentários específicos para `join_*`, `logic_join_*`, `loop_break_cont_*` e `loop_continue_cont_*`.
- Anotações de bloco em `join_*` e `logic_join_*` agora enfatizam retomada/continuação de fluxo.
- Sem alteração de semântica, Machine, lowering, parser, interpretador, opcodes, flags e `--selected`.

## Rodada documental estratégica (atual)
- Rodada atual dedicada a roadmap macro; **sem alterações funcionais** no compilador/runtime.
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- `docs/roadmap.md` criado como documento mestre de navegação estratégica (curto/médio/longo prazo) até uso geral, sistemas, self-hosting e kernel/bare metal.

## Situação operacional após a rodada
- Pipeline funcional permanece congelada e inalterada.
- `cargo build` e `cargo test` executados com sucesso nesta rodada.
- Sem abertura de nova fase funcional numerada.

## Fase 45 — aliases de tipo (`apelido`)
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- quarto item funcional do Bloco 1 entregue com escopo mínimo e auditável.
- parser/AST aceitam `apelido Nome = Tipo;` e tipos por alias em assinaturas e anotações já existentes.
- semântica resolve alias para tipo subjacente (sem nominalidade nova), rejeitando alias inexistente, duplicado e ciclos recursivos.
- lowering IR converte aliases para tipos concretos, preservando as camadas CFG/selected/Machine/runtime sem redesign.
- fora de escopo mantido: arrays, structs, ponteiros, casts (`virar`), backend `.s`, ABI e módulos.

## Fase 46 — arrays fixos (tipo estrutural mínimo)
- continuidade histórica da trilha funcional e das rodadas documentais verificada e preservada.
- quinto item funcional do Bloco 1 entregue com escopo mínimo e auditável.
- sintaxe de tipo adicionada: `[tipo; N]` em pontos tipados já existentes (alias, parâmetros, retorno, anotações).
- semântica valida tamanho estático simples (`N > 0`), resolve aliases no tipo-base e rejeita base inválida/inexistente.
- IR/lowering passou a carregar tipo de array fixo em assinaturas/slots sem introduzir operações de memória/indexação.
- fora de escopo mantido: inicializador literal de array, indexação, leitura/escrita por elemento, arrays dinâmicos, structs, ponteiros, casts e backend `.s`.


## Fase 49 — acesso a campo e indexação
- continuidade histórica preservada: Fase 48 permanece a fase funcional principal anterior e Fase 48-H1 permanece hotfix extraordinário sem reordenar roadmap.
- parser generalizado para cadeia postfix (`call`, `field access`, `index`) mantendo precedência existente de unários/binários.
- semântica valida leitura de campo em `ninho` e indexação em array fixo com índice inteiro.
- escrita em `obj.campo = ...` e `arr[i] = ...` não entrou nesta fase (LHS segue restrito a identificador).
- IR estruturada ganhou nós mínimos para representar campo/index; CFG/selected/machine/interpreter ainda não loweram esses nós por decisão de escopo.
- bounds-check: não introduzido nesta fase.
- próximo item normal do roadmap principal: Bloco 2, item 3 (`casts` controlados).

## Rodada documental — limpeza e normalização documental (paralela à Fase 51)

- Rodada documental sem número; executada em paralelo à próxima fase funcional sem conflito.
- `docs/handoff_auditor.md` formalmente abandonado por defasagem operacional (gap de 7 fases sem atualização após Fase 43). Arquivo preservado como legado histórico, marcado como descontinuado.
- `docs/future.md` normalizado como inventário amplo sem vínculo com fase específica:
  - referências a números de fase incorretos foram removidas/corrigidas;
  - itens já implementados riscados com `~~...~~`;
  - itens parcialmente implementados (cast `virar`, acesso a campo/indexação) marcados como "parcial" com indicação explícita do que falta;
  - seção "5 itens mais críticos" (tom de mini-roadmap) substituída por referência orientativa de distância sem conotação de ordem ativa;
  - convenção de status normalizada (✅ implementado / ⚠️ parcial / não iniciado / ideia futura).
- Precedência documental reafirmada: `docs/roadmap.md` = ordem ativa de execução; `docs/future.md` = inventário amplo de possibilidades.
- Nenhuma alteração funcional; nenhuma fase numerada aberta; nenhum conflito com fase paralela.
- `cargo build --locked` e `cargo test --locked` executados sem falhas ao final da rodada.

## Fase 50 — casts controlados
- continuidade histórica preservada: Fase 49 permanece a fase funcional principal anterior e Fase 48-H1 permanece rodada extraordinária/hotfix sem reordenar roadmap.
- sintaxe adicionada: `expr virar tipo` como cast explícito local na expressão.
- parser/AST/JSON/printer atualizados sem redesign amplo de precedência (cast aplicado como sufixo pós-unário, antes dos binários).
- política semântica mínima desta fase: apenas cast entre tipos inteiros (`bombom`, `u8/u16/u32/u64`, `i8/i16/i32/i64`), incluindo aliases resolvidos para tipo subjacente.
- casts com `logica`, `seta`, `ninho` e arrays fixos foram mantidos fora de escopo (erro semântico explícito).
- IR estruturada ganhou nó mínimo de cast com validação dedicada.
- decisão operacional desta fase: CFG/Machine/runtime ainda não loweram/executam cast; a falha é explícita no lowering de CFG.
- proteção de runtime signed (HF-3) foi preservada sem afrouxamento.
- próximo item normal do roadmap principal: Bloco 2, item 4 (`sizeof`/alinhamento).
