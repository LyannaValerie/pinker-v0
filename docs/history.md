# Linha do tempo da Pinker v0

Este arquivo é a crônica histórica única do projeto, separada por categoria.

============================== FASES ==============================

9 - disciplina de pilha da Machine
- Validação estrutural inicial da pilha na camada `--machine`.

10 - checagem leve de tipo no topo da pilha
- Regras mínimas de compatibilidade de tipos em operações de pilha.

11 - refinamento de tipos de params/slots na Machine
- Melhoria de inferência local para reduzir ambiguidades de tipo na validação.

12 - contexto e mensagens de erro da Machine
- Diagnósticos mais claros e contextualizados na camada de máquina abstrata.

13 - interpretador mínimo com `--run`
- Execução inicial de programas a partir da Machine validada.

14 - chamadas entre funções no interpretador
- Suporte a `call` e `call_void` no runtime.

15 - globals no interpretador
- Leitura de globais (`load_global`) no fluxo de execução.

16 - robustez do interpretador e testes negativos de runtime
- Consolidação de erros de execução e cobertura de cenários inválidos.

17 - recursão coberta por testes dedicados e exemplos CLI
- Cobertura de recursão em `--run` com testes e exemplos versionados.

18 - CI mínima + MSRV
- Esteira de qualidade inicial e versão mínima de Rust definida.

19 - padronização de mensagens entre IR/CFG/Machine
- Uniformização de formato diagnóstico nas camadas intermediárias.

20 - expansão de testes end-to-end com `--run`
- Maior cobertura de execução real no fluxo completo.

21 - stack trace simples de runtime
- Runtime passou a anexar rastreio de chamadas em erros.

22 - stack trace com mais contexto
- Enriquecimento de contexto por frame sem redesign amplo.

23 - ganchos leves para evolução do stack trace
- Estrutura preparada para contexto adicional futuro.

24 - mensagens de runtime com categorias estáveis
- Erros com prefixo categorizado e dicas curtas por tipo.

25 - renderização final de erro de runtime no CLI
- Saída de erro consolidada e previsível para usuário final.

26 - limite preventivo de recursão no runtime
- Proteção por profundidade máxima de chamadas no interpretador.

27 - `sempre que`
- Loop `while` da linguagem integrado ao pipeline.

28 - truncamento de stack trace longo
- Resumo de trace extenso para manter diagnóstico legível.

29 - `quebrar`
- Controle de fluxo para interrupção de loop.

30 - `continuar`
- Controle de fluxo para salto para próxima iteração.

31 - spans/source context melhores em erros
- Diagnóstico de parser/runtime com contexto de origem reforçado.

32 - consolidação de exemplos e cobertura CLI de loops
- Organização de suíte para `sempre que`, `quebrar` e `continuar`.

33 - cobertura negativa de loops + organização de backlog
- Casos inválidos de loop cobertos e backlog formalizado em `future.md`.

34 - operadores bitwise básicos
- Inclusão de `&`, `|`, `^`, `<<`, `>>` no pipeline.

35 - robustez de lowering CFG para `talvez/senao`
- Melhor tratamento de fall-through em ambos os ramos.

36 - operadores lógicos `&&` e `||` com short-circuit
- Semântica de curto-circuito integrada ao fluxo.

37 - licença do projeto e documentação de uso básico
- Projeto formalizado com licença e instruções essenciais.

38 - humanização da renderização de `--machine`
- Saída mais legível sem alterar semântica da máquina.

39 - comentários por instrução em `--machine`
- Camada textual recebeu explicações curtas por instrução.

40 - comentários de `--machine` mais contextuais
- Comentários passaram a refletir alvo/slot/uso de forma mais clara.

41 - comentários sensíveis ao papel de fluxo
- Renderização textual com contexto de controle aprimorado.

42 - operador `%` nativo
- Primeira fase funcional do Bloco 1 entregue no pipeline completo.

43 - inteiros unsigned fixos (`u8`, `u16`, `u32`, `u64`)
- Tipos unsigned com validação e integração em frontend/semântica/IR/runtime.

44 - inteiros signed fixos (`i8`, `i16`, `i32`, `i64`)
- Tipos signed integrados ao pipeline de linguagem.

45 - aliases de tipo (`apelido`)
- Declaração e resolução semântica para tipo subjacente.

46 - arrays fixos (`[T; N]`)
- Categoria de tipo estrutural mínima com validação básica de tamanho.

47 - structs (`ninho`)
- Declaração e validações semânticas centrais para tipo composto nomeado.

48 - ponteiros (`seta<T>`)
- Categoria de tipo ponteiro integrada ao pipeline sem memória operacional completa.

49 - acesso a campo e indexação (leitura)
- `obj.campo` e `arr[idx]` com escopo mínimo e sem escrita em LHS.

50 - cast explícito (`virar`)
- Cast controlado inteiro→inteiro no frontend/semântica/IR.

51 - `peso(tipo)` e `alinhamento(tipo)`
- Cálculo estático de layout/alinhamento com lowering para literal constante.

52 - `fragil seta<T>`
- Qualificador `volatile` semântico propagado no pipeline.

53 - backend textual `.s` inicial (`--asm-s`)
- Emissão assembly-like derivada de `selected` com subset escalar.

54 - ABI textual mínima interna no `.s`
- Contrato textual de argumentos/retorno e marcações estruturais por função.

55 - integração externa mínima com assembler/linker
- Prova experimental Linux x86_64 para subset estrito via testes.

56 - inline asm mínimo (`sussurro`)
- Statement textual preservado em frontend/semântica/IR.

57 - freestanding/no-std (`livre;`)
- Marca de unidade freestanding reconhecida no pipeline.

58 - boot entry/linker script textual mínimo
- Representação inicial de boot metadata em modo `livre`.

59 - primeiro kernel mínimo experimental
- Stub `_start` textual mínimo no fluxo freestanding.

60 - módulos/imports (`trazer`)
- Import de módulo e símbolo no mesmo diretório do arquivo raiz.

61 - strings (`verso`)
- Tipo/literal de string integrado ao frontend/semântica/IR.

62 - I/O básico (`falar`)
- Saída mínima em `--run` para subset tipado definido.

63 - `pink build`
- Comando de projeto para gerar artefato textual `.s` em disco.

64 - signed real no runtime
- `--run` passou a executar família `i8..i64` com representação signed explícita.

65 - representação mínima de ponteiro no runtime
- Runtime passou a diferenciar ponteiro (`Ptr`) de inteiro escalar.

66 - dereferência de leitura
- Sintaxe de leitura indireta `*expr` integrada ao frontend/semântica/IR/CFG/selected/Machine/runtime.
- Subset operacional desta fase: apenas `seta<bombom>`; demais bases de ponteiro seguem fora de escopo.
- Runtime ganhou `deref_load` com memória abstrata mínima baseada em endereços de globals escalares (`eterno`) para suportar `--run`.
- Literais inteiros passaram a ser aceitos como endereço para inicialização de `seta<T>` nesta fase bootstrap.

67 - escrita indireta
- Sintaxe de escrita indireta `*expr = valor;` integrada ao frontend/semântica/IR/CFG/selected/Machine/runtime.
- Subset operacional desta fase: apenas escrita em `seta<bombom>`; demais bases de ponteiro seguem fora de escopo.
- Runtime ganhou `deref_store` com atualização na memória abstrata mínima baseada em endereços de globals escalares já mapeadas.
- Política de erro explícita em `--run`: escrita falha para endereço inválido/não inicializado e para valor incompatível com o tipo esperado.

68 - aritmética de ponteiros
- Aritmética mínima de ponteiros integrada ao frontend/semântica/runtime para o subset operacional `seta<bombom> ± bombom`.
- Operações suportadas nesta fase: `ptr + n` e `ptr - n` com `ptr: seta<bombom>` e `n: bombom`; resultado permanece ponteiro utilizável por `*p` e `*p = valor`.
- Operações fora de escopo explícitas nesta fase: `n + ptr`, `ptr - ptr`, comparação rica de ponteiros e bases diferentes de `bombom`.
- Semântica de deslocamento desta fase: offset em unidades lógicas do modelo de memória abstrata vigente (endereços de globals já mapeadas), sem escala por tamanho de tipo base.

69 - acesso a campo operacional em `ninho`
- Acesso operacional mínimo de campo integrado ao pipeline até `--run` para leitura em `ninho` usando offsets do layout estático.
- Superfície funcional desta fase: **leitura de campo** (não inclui escrita de campo).
- Subset operacional desta fase: leitura via `(*ptr).campo` com `ptr: seta<ninho>` e campo escalar (`bombom`, `u8..u64`, `i8..i64`, `logica`).
- Caminho de lowering desta fase: `FieldAccess` usa offset estático de `layout` + aritmética de ponteiro + `deref_load` para ler o campo em runtime.
- Fora de escopo explícito nesta fase: acesso por valor (`p.campo`), escrita de campo, indexação operacional plena e campos não escalares.

70 - indexação operacional em arrays
- Indexação operacional mínima integrada ao pipeline até `--run` para **leitura por índice** em arrays no modelo de memória atual.
- Superfície funcional desta fase: **leitura por índice** (escrita por índice não entrou nesta rodada).
- Subset operacional desta fase: `(*ptr)[i]` com `ptr: seta<[bombom; N]>` e `i: bombom`.
- Caminho de lowering desta fase: `Index` usa ponteiro base + índice como offset (unidades lógicas do runtime atual, sem escala adicional) + `deref_load`.
- Relação com fases anteriores: reutiliza aritmética de ponteiros (Fase 68) e leitura indireta `deref_load` (Fase 66); mantém escrita indireta (Fase 67) sem estender para `arr[i] = ...`.
- Fora de escopo explícito nesta fase: base por valor (`arr[i]`), escrita por índice, elementos não `bombom`, arrays gerais e checks sofisticados de bounds.

71 - cast operacional útil ligado à memória
- `virar` passou a ter lowering operacional em CFG/selected/Machine/runtime para subset mínimo útil de memória.
- Casts suportados nesta fase: inteiro->inteiro (já existente) e `bombom <-> seta<bombom>`.
- Semântica operacional desta fase: reinterpretação direta do endereço lógico do runtime (sem MMIO, sem reinterpretação ampla de memória, sem cast geral entre compostos).
- Cenário destravado: bootstrap de ponteiro deixa de depender exclusivamente de literal cru em alguns fluxos (`bombom` pode virar `seta<bombom>` e voltar para `bombom` em `--run`).
- Fora de escopo explícito nesta fase: `bombom -> seta<T>` genérico, `seta<T> -> bombom` para `T != bombom`, casts gerais entre ponteiros/compostos, backend nativo de cast.

72 - efeito operacional mínimo de `fragil`
- `fragil` deixou de ser apenas qualificador semântico: a volatilidade passa a ser propagada explicitamente em IR/CFG/selected/Machine/runtime nos acessos indiretos.
- `deref_load` e `deref_store` agora carregam metadata `is_volatile` e têm caminhos operacionais distintos no pipeline (`deref_*_fragil` vs `deref_*`), com validação de consistência entre tipo do ponteiro e instrução.
- Subset operacional desta fase: `fragil seta<bombom>` em leitura/escrita indireta (`*p` e `*p = valor`) no `--run`, reaproveitando o modelo de memória abstrata já existente.
- Fora de escopo explícito nesta fase: MMIO real, hardware real, fences/barreiras, ordenação de memória e ampliação para outros tipos base além do subset já aceito para dereferência/escrita indireta.

73 - subset real montável ampliado
- Primeira fase funcional do Bloco 7 — Backend nativo real.
- Integração externa (`emit_external_toolchain_subset`) deixou de aceitar apenas retorno constante e passou a aceitar subset linear maior em Linux x86_64: `principal() -> bombom` com locais `bombom`, atribuição local e aritmética escalar `+`, `-`, `*`.
- Emissão externa agora gera prólogo/epílogo real mínimo de frame (`pushq %rbp`, `movq %rsp, %rbp`, `leave`) e slots de stack para locais/temporários, mantendo recorte pequeno e auditável.
- Fluxo real assembler/linker coberto por teste dedicado com execução do binário resultante (exit code validado) para caso com múltiplas instruções e retorno calculado.
- Fora de escopo explícito nesta fase: parâmetros, globais, fluxo de controle (`talvez/senão`, loops), chamadas, memória indireta e ABI completa.

74 - convenção de chamada concreta mínima
- Segunda fase funcional do Bloco 7 — Backend nativo real.
- Integração externa (`emit_external_toolchain_subset`) passou a aceitar **chamada direta real** no subset Linux x86_64 hospedado, com múltiplas funções `-> bombom` em bloco único linear.
- Convenção concreta mínima desta fase: retorno em `%rax`; passagem de **um argumento `bombom`** em `%rdi`; `principal` segue mapeada para `main`.
- Subset garantido desta fase: `principal() -> bombom` chamando função auxiliar com 0 ou 1 argumento `bombom`, com atribuição/local e aritmética linear (`+`, `-`, `*`) antes/depois da call.
- Fora de escopo explícito nesta fase: mais de 1 argumento, chamadas complexas, recursão externa, fluxo de controle, globais, memória indireta/ponteiros e ABI completa.

%%%%%%%%%%%%%%%%%%%%%%%%%%%% HOTFIXES %%%%%%%%%%%%%%%%%%%%%%%%%%%%

HF-1 - Fase 48-H1: hotfixes de corretude e manutenção
- Pacote extraordinário após a Fase 48, sem reordenar a trilha funcional.
- Corretude central: comparação estrutural de tipos ignorando spans, erro de runtime com span opcional, bloqueios/diagnósticos defensivos de runtime e validação estrita de range de literais.
- Manutenção central: simplificações de CLI, alinhamento de toolchain/CI com MSRV, inclusão de `clippy` e validação de docs na esteira.
- Higiene documental: atualização de backlog e normalização de registros associados ao ciclo de hotfix.

HF-2 - Bloco 6 (Fases 64–70): varredura de corretude e estabilização
- Pacote extraordinário pós-Bloco-6 sem avançar trilha funcional.
- Bug #1 (interpreter.rs): `normalize_numeric_pair` invertia a ordem dos operandos quando `Int` era LHS e `IntSigned` era RHS, devido a padrão `|` com bindings compartilhados. Corrigido separando em dois arms explícitos que preservam a ordem lhs/rhs original. Efeito observado: `10 - 3 = -7`, `5 < 3 = verdade`.
- Bug #2 (interpreter.rs): Classificador de erros de runtime não reconhecia erros de ponteiro (`deref_load`, `deref_store`, `endereço inválido`, `ponteiro no topo`). Hint diagnóstico adicionado.
- Bug #3 (semantic.rs): Verificação redundante morta de tipo de índice em `ExprKind::Index` (subsumed pela checagem subsequente `matches!(bombom)`). Código morto removido.
- Bug #4 (ir_validate.rs + cfg_ir_validate.rs): `Eq/Neq` rejeitava `signed_var == literal` por ausência da exceção literal já presente em `Lt/Lte/Gt/Gte`. Corrigido em ambas as camadas de validação.
- Teste de regressão adicionado: `run_signed_literal_lhs_operacoes_nao_comutativas`.
- Suite completa: 356 testes, 0 falhas.

########################## DOCUMENTAÇÃO ##########################

Doc-1 - viabilidade de escrita em globals (análise)
- Rodada documental sem mudança funcional.
- Conclusão registrada: escrita em globals permaneceu fora do escopo naquele estado.

Doc-2 - auditoria de duplicação e revalidação operacional
- Rodada documental sem mudança funcional.
- Verificação de duplicações e rechecagem de saúde do projeto registradas.

Doc-3 - doc comments estruturais em módulos centrais
- Rodada documental sem mudança funcional.
- Comentários estruturais e organização textual aprimorados.

Doc-4 - consolidação da trilha única em `roadmap.md`
- `roadmap.md` formalizado como trilha ativa oficial.
- Separação explícita com `future.md` registrada.

Doc-5 - normalização documental paralela à Fase 51
- `future.md` normalizado como inventário amplo sem ditar ordem ativa.
- Registro de abandono operacional de handoff legado.

Doc-6 - criação de `docs/parallel.md`
- Inclusão do documento visionário da Pinker sem transformar em backlog.
- Precedência documental entre `roadmap`/`future`/`parallel` reforçada.

Doc-7 - abertura documental do Bloco 6
- Após fechamento do Bloco 5 (Fase 63), Bloco 6 foi oficializado como trilha ativa.
- Próximo eixo funcional consolidado: memória operacional.

Doc-8 - reestruturação documental e regras obrigatórias
- `phases.md` reorganizado em seções formais: FASES / HOTFIXES / DOCUMENTAÇÃO.
- `agent_state.md` enxugado para estado corrente e diretrizes operacionais.
- `handoff_codex.md` reduzido para handoff curto da rodada.
- `doc_rules.md` criado como referência obrigatória de convenções.
- `handoff_auditor.md` e `handoff_opus.md` removidos por legado descontinuado sem conteúdo único ativo.

Doc-9 - revisão documental operacional da reestruturação
- Auditoria de coerência entre `roadmap.md`, `future.md`, `parallel.md`, `phases.md`, `agent_state.md`, `handoff_codex.md` e `README.md`.
- Validação de precedência documental e continuidade histórica sem abrir nova fase funcional.
- Ajuste textual mínimo em `parallel.md` para correção de digitação, sem impacto operacional.

Doc-10 - renomeação de `phases.md` para `history.md`
- Rodada estritamente documental; sem alteração funcional de código, testes ou exemplos.
- Arquivo `docs/phases.md` renomeado para `docs/history.md`; conteúdo histórico preservado integralmente.
- Objetivo: alinhar o nome do arquivo ao seu papel real de crônica histórica única do projeto.
- Referências atualizadas em: `README.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/doc_rules.md`, `docs/parallel.md`.
- Trilha ativa do roadmap permaneceu intacta; `roadmap.md` não foi alterado.
- Arquitetura documental mantida: `roadmap.md` = trilha ativa, `future.md` = inventário amplo, `parallel.md` = visão orientadora, `history.md` = crônica histórica, `agent_state.md` = estado corrente, `handoff_codex.md` = handoff operacional curto.

Doc-11 - abertura documental dos Blocos 7 e 8
- Rodada estritamente documental; sem alteração funcional de código, testes ou exemplos `.pink`.
- Objetivo: eliminar ambiguidade estratégica sobre o próximo grande rumo do projeto após o fechamento do Bloco 6.
- Bloco 6 — Memória operacional marcado como concluído no roadmap (Fases 64–72 entregues).
- **Bloco 7 — Backend nativo real** registrado formalmente como trilha ativa seguinte: transformar gradualmente o backend textual/experimental em backend nativo real mais utilizável.
- **Bloco 8 — I/O e ecossistema útil** registrado formalmente como bloco futuro já definido, não ativo: I/O, arquivo, verso operacional e base para tooling.
- Separação explícita preservada: trilha de soberania/backend (Bloco 7) vs. trilha de I/O/ecossistema (Bloco 8).
- Trilha ativa permanece única: apenas o Bloco 7 está marcado como próximo bloco ativo; o Bloco 8 aguarda consolidação suficiente do Bloco 7.
- Esta rodada não cria fase funcional; registra apenas a direção planejada.
