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

%%%%%%%%%%%%%%%%%%%%%%%%%%%% HOTFIXES %%%%%%%%%%%%%%%%%%%%%%%%%%%%

HF-1 - Fase 48-H1: hotfixes de corretude e manutenção
- Pacote extraordinário após a Fase 48, sem reordenar a trilha funcional.
- Corretude central: comparação estrutural de tipos ignorando spans, erro de runtime com span opcional, bloqueios/diagnósticos defensivos de runtime e validação estrita de range de literais.
- Manutenção central: simplificações de CLI, alinhamento de toolchain/CI com MSRV, inclusão de `clippy` e validação de docs na esteira.
- Higiene documental: atualização de backlog e normalização de registros associados ao ciclo de hotfix.

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
