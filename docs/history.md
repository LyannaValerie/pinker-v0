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

75 - frame/registradores mínimos reais
- Terceira fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou o recorte da Fase 74 e passou a adotar disciplina mínima explícita de registradores: `%rax` (acumulador/retorno), `%rdi` (argumento único) e `%r10` (temporário volátil de binárias lineares).
- Emissão de aritmética/call foi consolidada para reduzir casos ad hoc e manter consistência do frame mínimo por função com `%rbp` e slots de parâmetro/local/temporários.
- Cobertura externa real ampliada com caso versionado dedicado da Fase 75 exercitando call + retorno + locals/aritmética sob a disciplina de frame/registradores.
- Fora de escopo explícito nesta fase: register allocation amplo, ABI final de plataforma, mais de 1 parâmetro, fluxo de controle geral e memória indireta/ponteiros no backend externo.

76 - múltiplos parâmetros mínimos reais
- Quarta fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável ampliou chamadas diretas de até 1 para **até 2 parâmetros `bombom`**, com convenção concreta mínima no recorte Linux x86_64 hospedado.
- Convenção desta fase no subset: `%rdi` (arg0), `%rsi` (arg1), `%rax` (retorno/acumulador) e `%r10` (temporário volátil de binárias), preservando frame mínimo com `%rbp` e slots lineares.
- Integração externa real coberta com caso versionado dedicado exercitando compilação/linkedição/execução de função com 2 parâmetros e retorno calculado.
- Fora de escopo explícito nesta fase: 3+ parâmetros, parâmetros não `bombom`, recursão externa, fluxo de controle, globais, memória indireta/ponteiros no backend externo e ABI completa.

77 - memória real mínima no backend
- Quinta fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou o recorte da Fase 76 e passou a declarar/cobrir explicitamente o primeiro acesso de memória real mínimo no backend externo: load/store em slots de frame via `%rbp`.
- Emissão externa no recorte continua Linux x86_64 hospedado, com frame mínimo (`%rbp`) e registradores do subset (`%rax`, `%rdi`, `%rsi`, `%r10`), agora com cobertura dedicada para fluxo real que depende de leitura e escrita em memória de frame.
- Integração externa real coberta por exemplo/teste versionado da fase, compilando/montando/linkando/executando e validando resultado observável.
- Fora de escopo explícito nesta fase: memória indireta geral/ponteiros (`*p`), arrays/structs operacionais no backend externo, globais, fluxo de controle geral e ABI completa.


78 - composição linear interprocedural mais rica
- Sexta fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 77 e ampliou o artefato executável real com composição interprocedural linear mais rica: encadeamento de chamadas diretas em múltiplos níveis no mesmo executável, reaproveitando `%rdi`/`%rsi`, `%rax`, `%r10` e slots de frame via `%rbp`.
- Cobertura externa real dedicada da fase com exemplo versionado novo e teste de integração que compila/monta/linka/executa fluxo `principal -> combina -> (soma2/ajusta)` e valida resultado observável.
- Fora de escopo explícito nesta fase: controle de fluxo geral, memória indireta/ponteiros no backend externo, globais, 3+ parâmetros, parâmetros não `bombom`, recursão externa e ABI completa.

79 - programa linear maior com mais etapas
- Sétima fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 78 e formalizou cobertura para programa linear mais denso no mesmo executável: mais atribuições intermediárias, maior reuso de slots de frame e mais etapas aritméticas lineares entre chamadas diretas já suportadas.
- Cobertura externa real ampliada com exemplo/teste versionado da fase (`etapa/refina/principal`) exercitando compilação/montagem/linkedição/execução e validação de resultado observável em fluxo linear mais comprido.
- Fora de escopo explícito nesta fase: controle de fluxo geral, memória indireta/ponteiros no backend externo, globais, 3+ parâmetros, parâmetros não `bombom`, recursão externa e ABI completa.

80 - cobertura linear auditável mais ampla
- Oitava fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 79 e formalizou cobertura adicional para combinação linear mais rica entre densidade local e composição interprocedural no mesmo executável.
- Cobertura externa real ampliada com exemplo/teste versionado da fase (`base/mistura/principal`) exercitando compilação/montagem/linkedição/execução e validação de resultado observável em fluxo linear maior com reuso de chamadas diretas já suportadas.
- Fora de escopo explícito nesta fase: controle de fluxo geral, memória indireta/ponteiros no backend externo, globais, 3+ parâmetros, parâmetros não `bombom`, recursão externa e ABI completa.


81 - recusa explícita complementar no subset externo
- Nona fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 80 e passou a rejeitar de forma explícita e auditável funções/calls com 3+ parâmetros no fluxo `--asm-s`.
- Diagnósticos do backend externo foram endurecidos para declarar o limite garantido do contrato (até 2 parâmetros `bombom`) e evitar leitura incidental de suporte além do subset.
- Cobertura de teste ampliada com caso versionado dedicado da fase validando a recusa explícita de 3+ parâmetros.
- Fora de escopo explícito nesta fase: abertura de novos fundamentos (controle de fluxo geral, memória indireta/ponteiros, globais, parâmetros não `bombom` e ABI completa).

82 - recusa explícita complementar de controle de fluxo no subset externo
- Décima fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 81 e passou a rejeitar de forma explícita e auditável `talvez/senão` no fluxo `--asm-s`.
- Diagnóstico do backend externo foi endurecido para separar com clareza o subset linear garantido da proximidade estrutural de controle de fluxo geral.
- Cobertura de teste ampliada com caso versionado dedicado da fase validando a recusa explícita de `talvez/senão` no backend externo.
- Fora de escopo explícito nesta fase: abertura de lowering de branches/labels/saltos, loops no backend externo, memória indireta/ponteiros, globais, parâmetros não `bombom` e ABI completa.

83 - matriz de fronteira auditável do subset externo
- Décima primeira fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 82 e consolidou uma matriz mínima auditável de fronteira (garantido vs rejeitado), sem abrir fundamentos novos.
- Contrato textual e diagnósticos do backend externo foram alinhados para rotular explicitamente a Fase 83 mantendo o mesmo subset linear já suportado.
- Cobertura de teste ampliada com caso agregador dedicado da fase exercitando: caso positivo representativo (linear), caso positivo interprocedural, caso positivo com memória mínima de frame, recusa explícita de 3+ parâmetros e recusa explícita de `talvez/senão`.
- Fora de escopo explícito nesta fase: controle de fluxo geral, memória indireta/ponteiros, globais amplas, ABI completa, recursão externa e abertura do Bloco 8.

84 - recusa explícita complementar de `sempre que` no subset externo
- Décima segunda fase funcional do Bloco 7 — Backend nativo real.
- Subset externo montável preservou integralmente o recorte da Fase 83 e passou a rejeitar de forma explícita e auditável `sempre que` no fluxo `--asm-s`.
- Diagnóstico do backend externo foi endurecido para separar de forma objetiva o subset linear garantido da proximidade estrutural de loops.
- Cobertura de teste ampliada com caso versionado dedicado da fase validando a recusa explícita de `sempre que`, mantendo as recusas explícitas já existentes de `talvez/senão` e 3+ parâmetros.
- Fora de escopo explícito nesta fase: abertura de lowering de loops/branches/labels/saltos no backend externo, memória indireta/ponteiros, globais, parâmetros não `bombom`, ABI completa e abertura do Bloco 8.

85 - entrada básica com `ouvir` em `--run`
- Primeira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado: intrínseca `ouvir()` no runtime interpretado (`--run`) com retorno `bombom` (u64) e aridade fixa zero.
- Pipeline alinhada no recorte: semântica/IR/validações (IR, CFG IR e Machine) passaram a reconhecer `ouvir` como intrínseca válida sem declaração explícita de função.
- Runtime passou a ler uma linha de `stdin`, interpretar como `bombom` e falhar com diagnóstico explícito em entrada inválida (incluindo entrada vazia).
- Cobertura adicionada com exemplo versionado da fase (`examples/fase85_ouvir_bombom_valido.pink`) e testes automatizados para caso positivo e erro de parse em `--run`.
- Fora de escopo explícito nesta fase: arquivo (`abrir`/`fechar`/`escrever`), `verso` operacional amplo, múltiplos tipos de entrada e backend externo para I/O.

86 - leitura mínima de arquivo com `abrir`/`fechar` em `--run`
- Segunda fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado no runtime interpretado: `abrir("caminho") -> bombom` (handle), `ler_arquivo(handle) -> bombom` (leitura simples de conteúdo inteiro) e `fechar(handle)` (encerramento explícito).
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações passaram a reconhecer `abrir`, `ler_arquivo` e `fechar` como intrínsecas sem declaração explícita de função.
- Diagnósticos claros adicionados para caminho inválido/falha de abertura, handle inválido e conteúdo de arquivo não parseável como `bombom` (u64).
- Cobertura adicionada com exemplo versionado da fase (`examples/fase86_arquivo_leitura_minima_valido.pink`) e testes automatizados de sucesso e falha em `--run`.
- Fora de escopo explícito nesta fase: `escrever`, múltiplos modos de abertura, streaming, diretórios, leitura textual ampla e `verso` operacional geral.

87 - escrita mínima de arquivo com `escrever` em `--run`
- Terceira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado no runtime interpretado: `escrever(handle, bombom)` para sobrescrever o conteúdo do arquivo já aberto por `abrir("caminho")`, mantendo `ler_arquivo(handle)` e `fechar(handle)` no mesmo fluxo de handle.
- Superfície escolhida nesta fase: **Opção A** (`abrir` retorna handle, `escrever` sem retorno, `fechar` explícito), preservando a API mínima anterior e evitando inflar status de subsistema de arquivos.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações passaram a reconhecer `escrever` como intrínseca sem declaração explícita de função.
- Diagnósticos claros adicionados para uso incorreto de `escrever` (aridade, tipo de handle/valor, handle inválido e falha de escrita no arquivo).
- Cobertura adicionada com exemplo versionado da fase (`examples/fase87_arquivo_escrita_minima_valido.pink`) e testes automatizados de sucesso/falha incluindo integração escrita+leitura.
- Fora de escopo explícito nesta fase: append/modos de abertura, diretórios, streaming, escrita textual ampla e `verso` operacional geral.

88 - `verso` operacional útil mínimo em `--run`
- Quarta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado no runtime interpretado: `verso` como valor operacional em variável local, passagem por chamada, retorno e `falar(verso)` por valor (não apenas literal).
- Pipeline alinhada no recorte: CFG IR passou a lowerar `ValueIR::String` para `OperandIR::Str` em expressões/locals/chamadas/retornos; Machine/runtime ganharam instrução de impressão de `verso` por valor de pilha.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase88_verso_operacional_minimo_valido.pink`) e testes automatizados positivos em `--run` e `--cfg-ir`.
- Limite explícito preservado: `eterno` global de `verso` continua fora do subset operacional da CFG IR nesta fase; concatenação/comprimento/indexação também seguem fora.

89 - operações mínimas de texto úteis em `verso` (`juntar_verso` + `tamanho_verso`)
- Quinta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado no runtime interpretado: concatenação via intrínseca `juntar_verso(verso, verso) -> verso` e comprimento via intrínseca `tamanho_verso(verso) -> bombom`.
- Superfície escolhida nesta fase: intrínsecas pontuais (sem abrir operador textual novo), priorizando diff pequeno e auditável.
- Pipeline alinhada no recorte: semântica/CFG IR/selected/Machine/validações passaram a reconhecer as duas intrínsecas sem declaração explícita de função.
- Runtime passou a executar concatenação mínima de `verso` e cálculo de comprimento em contagem de caracteres Unicode (`chars().count()`), retornando `bombom`.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase89_verso_operacoes_minimas_valido.pink`) e testes automatizados de semântica/`--run`/CLI.
- Fora de escopo explícito nesta fase: indexação de `verso`, slicing, interpolação/formatação e biblioteca textual ampla.

90 - indexação mínima de `verso` com fronteira auditável (`indice_verso`)
- Sexta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Recorte mínimo implementado no runtime interpretado: intrínseca `indice_verso(verso, bombom) -> verso`, retornando `verso` unitário (1 caractere).
- Superfície escolhida nesta fase: intrínseca pontual (`indice_verso`) para evitar expansão de gramática e manter diff pequeno/auditável.
- Fronteira auditável explícita desta fase: tipo inválido é rejeitado na semântica; índice fora da faixa falha em runtime com diagnóstico claro.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações passaram a reconhecer `indice_verso` sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase90_verso_indexacao_minima_valido.pink`) e testes automatizados de semântica/`--run`/CLI.
- Fora de escopo explícito nesta fase: sintaxe `v[i]` para `verso`, slicing, indexação negativa, interpolação/formatação e biblioteca textual ampla.

91 - melhorias mínimas em `falar` (múltiplos argumentos + mistura heterogênea mínima)
- Sétima fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfície escolhida nesta fase: manter `falar(...)` e ampliar apenas sua aridade no runtime `--run`, sem criar API nova de formatação.
- Recorte mínimo implementado: `falar` aceita múltiplos argumentos com separação previsível por espaço simples e quebra de linha única ao fim da chamada.
- Mistura heterogênea mínima coberta no recorte: argumentos de tipos já estáveis (incluindo `bombom` + `verso`) na mesma chamada.
- Pipeline alinhada no recorte: AST/semântica/IR/CFG IR/selected/Machine/backend textual passaram a transportar lista de argumentos para `falar`.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase91_falar_multiplos_argumentos_valido.pink`) e testes automatizados de `--run`/CLI para múltiplos `bombom`, mistura `verso`+`bombom` e integração com local/chamada.
- Fora de escopo explícito nesta fase: interpolação, placeholders posicionais/nomeados, largura/alinhamento/precisão e biblioteca de formatação ampla.

92 - base mínima para tooling em `--run` (argv posicional + status explícito)
- Oitava fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfície escolhida para argumentos: intrínseca mínima `argumento(bombom) -> verso`, sem coleção/iterador amplo de argv.
- Superfície escolhida para status/código: intrínseca mínima `sair(bombom)` para encerrar com código explícito; sem parser de flags/subcomandos/env.
- CLI `pink --run` passou a aceitar repasse posicional via separador `--`, encaminhando os argumentos para `argumento(i)`.
- Pipeline alinhada no recorte: semântica e runtime reconheceram `argumento`/`sair` sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase92_tooling_base_argumento_status_valido.pink`) e testes automatizados de semântica/`--run`/CLI (casos positivos e limite de argumento ausente).
- Fora de escopo explícito nesta fase: parser de flags, subcomandos, env vars, diretórios, processos externos e biblioteca ampla de tooling.

93 - ergonomia mínima de argv em `--run` (contagem + presença por índice)
- Nona fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfície escolhida para contagem: intrínseca `quantos_argumentos() -> bombom`, sem coleção ampla de argv.
- Superfície escolhida para presença por índice: intrínseca `tem_argumento(bombom) -> logica`, para guarda mínima antes de `argumento(i)`.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `quantos_argumentos` e `tem_argumento` sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase93_argv_ergonomia_minima_valido.pink`) e testes automatizados de semântica/`--run`/CLI incluindo integração com `argumento(i)`.
- Fora de escopo explícito nesta fase: parser de flags, subcomandos, env vars, coleção/iteração ampla de argv, diretórios e processos externos.

94 - refinamento mínimo de argv em `--run` (fallback posicional simples)
- Décima fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfície escolhida para fallback: intrínseca mínima `argumento_ou(bombom, verso) -> verso`, mantendo foco em script pequeno sem coleção ampla.
- Semântica operacional desta fase: quando o índice existe em argv, retorna o argumento real; quando não existe, retorna o `verso` padrão fornecido.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `argumento_ou` sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado da fase (`examples/fase94_argumento_ou_fallback_minimo_valido.pink`) e testes automatizados de semântica/`--run`/CLI cobrindo ausência/presença.
- Fora de escopo explícito nesta fase: parser de flags, subcomandos, env vars, coleção/iteração ampla de argv, diretórios e processos externos.

95 - ambiente mínimo de processo em `--run` (fallback de env + diretório atual)
- Décima primeira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `ambiente_ou(verso, verso) -> verso` para leitura de variável de ambiente com fallback e `diretorio_atual() -> verso` para leitura do diretório corrente do processo.
- Semântica operacional desta fase: `ambiente_ou` retorna valor real quando a chave existe no ambiente do processo e usa o padrão quando a variável não está disponível; `diretorio_atual` expõe o caminho corrente como `verso`.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplos versionados (`examples/fase95_ambiente_processo_minimo_valido.pink`, `examples/fase95_diretorio_atual_minimo_valido.pink`, `examples/fase95_argumento_ou_ambiente_ou_valido.pink`) e testes automatizados de semântica/`--run`/CLI para fallback, leitura de ambiente real e integração com `falar`.
- Fora de escopo explícito nesta fase: mutação/listagem de env vars, mudança de diretório, listagem de diretórios, API ampla de paths e processos externos.

96 - introspecção mínima de caminho em `--run`
- Décima segunda fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `caminho_existe(verso) -> logica` para existência mínima de caminho e `e_arquivo(verso) -> logica` para classificação mínima no mesmo recorte.
- Semântica operacional desta fase: ambas as intrínsecas recebem `verso`; `caminho_existe` retorna presença do caminho informado e `e_arquivo` classifica se o caminho existente é arquivo regular.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase96_introspeccao_caminho_minima_valido.pink`) e testes automatizados de semântica/`--run`/CLI para caso positivo, caso negativo e integração com `diretorio_atual`/`falar`.
- Fora de escopo explícito nesta fase: listagem de diretórios, `e_diretorio`, mudança de diretório, globbing, mutação de paths, processos externos e biblioteca ampla de filesystem.

97 - refinamento mínimo de caminho em `--run`
- Décima terceira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `e_diretorio(verso) -> logica` para classificação complementar de caminho e `juntar_caminho(verso, verso) -> verso` para composição mínima baseada na stdlib de paths.
- Semântica operacional desta fase: `e_diretorio` retorna `verdade` apenas quando o caminho existe e é diretório; `juntar_caminho` compõe base+trecho sem prometer canonicalização nem normalização rica.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase97_refinamento_caminho_minimo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para casos positivos, negativos e integração com `diretorio_atual`/`argumento_ou`/`caminho_existe`/`falar`.
- Fora de escopo explícito nesta fase: canonicalização, normalização rica, listagem de diretórios, `chdir`, globbing, mutação ampla de paths, processos externos e biblioteca ampla de filesystem.

98 - refinamento mínimo de arquivo em `--run`
- Décima quarta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `tamanho_arquivo(verso) -> bombom` para tamanho mínimo de arquivo e `e_vazio(verso) -> logica` para teste mínimo de vazio.
- Semântica operacional desta fase: ambas as intrínsecas exigem `verso` apontando para arquivo regular existente; caminho ausente/não-arquivo falha com diagnóstico explícito de runtime.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase98_refinamento_arquivo_minimo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para tamanho positivo, vazio positivo, negativo de caminho ausente e integração com `argumento_ou`/`juntar_caminho`.
- Fora de escopo explícito nesta fase: timestamps, permissões, ownership, criação/remoção de arquivo, listagem de diretórios, leitura incremental, metadados ricos e biblioteca ampla de filesystem.

99 - refinamento mínimo de mutação de filesystem em `--run`
- Décima quinta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `criar_diretorio(verso) -> nulo` para criação mínima de diretório simples e `remover_arquivo(verso) -> nulo` para remoção mínima de arquivo simples.
- Semântica operacional desta fase: ambas as intrínsecas exigem caminho em `verso`; `criar_diretorio` usa criação não recursiva e falha em caminho inválido/existente; `remover_arquivo` remove apenas arquivo e falha para diretório/ausência.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase99_refinamento_diretorio_arquivo_minimo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para caso positivo de criação, caso positivo de remoção, caso negativo de tipo/caminho e integração com `argumento_ou`/`juntar_caminho`/`caminho_existe`/`e_arquivo`/`e_diretorio`.
- Fora de escopo explícito nesta fase: criação recursiva (`create_dir_all`), remoção de diretório, rename/move/cópia, listagem de diretórios, globbing e biblioteca ampla de filesystem.

100 - remoção mínima complementar de diretório + leitura textual mínima de arquivo em `--run`
- Décima sexta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `remover_diretorio(verso) -> nulo` para remoção mínima de diretório vazio e `ler_verso_arquivo(handle) -> verso` para leitura textual mínima do conteúdo completo de handle aberto via `abrir(...)`.
- Semântica operacional desta fase: `remover_diretorio` usa remoção não-recursiva (`remove_dir`) e falha para diretório não-vazio/caminho inválido; `ler_verso_arquivo` reutiliza handle existente e retorna `verso` integral sem streaming/append.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase100_refinamento_diretorio_texto_minimo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para remoção positiva de diretório vazio, negativo de diretório não-vazio, leitura textual positiva e integração com `argumento_ou`/`juntar_caminho`.
- Fora de escopo explícito nesta fase: remoção recursiva, rename/move/cópia, listagem de diretórios, leitura incremental/streaming, append, encoding sofisticado e biblioteca ampla de filesystem/texto.

101 - escrita textual mínima de arquivo em `--run`
- Décima sétima fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `escrever_verso(handle, verso) -> nulo` para escrita textual mínima e `criar_arquivo(verso) -> bombom` como complemento operacional mínimo para criação + handle no mesmo fluxo.
- Semântica operacional desta fase: `escrever_verso` sobrescreve o conteúdo textual completo do handle aberto (sem append/streaming) e `criar_arquivo` cria arquivo vazio em caminho informado e já retorna handle válido.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase101_escrita_textual_minima_arquivo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para escrita positiva, releitura positiva, negativo de handle inválido e integração com `argumento_ou`/`juntar_caminho`.
- Fora de escopo explícito nesta fase: append, streaming, escrita por linha, encoding sofisticado, rename/move/cópia, listagem de diretórios e biblioteca ampla de filesystem/texto.

102 - truncamento mínimo de arquivo em `--run`
- Décima oitava fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfície escolhida: `truncar_arquivo(handle) -> nulo` para truncamento mínimo por handle já aberto, sem abrir superfície paralela por caminho.
- Semântica operacional desta fase: `truncar_arquivo` exige handle `bombom` válido e aberto; em sucesso, zera o conteúdo do arquivo e do buffer em runtime para manter consistência imediata no mesmo handle.
- Integração explícita do pós-estado: validação mínima acoplada via `tamanho_arquivo(verso) -> bombom` e `e_vazio(verso) -> logica`, com releitura textual mínima por `ler_verso_arquivo(handle)` retornando `verso` vazio após truncamento.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `truncar_arquivo` sem declaração explícita de função.
- Cobertura adicionada com exemplo versionado (`examples/fase102_truncamento_minimo_arquivo_valido.pink`) e testes automatizados de semântica/`--run`/CLI para caso positivo, pós-estado explícito (`tamanho_arquivo` + `e_vazio` + releitura) e negativos de handle inválido/já fechado.
- Fora de escopo explícito nesta fase: truncamento por caminho, append, streaming, escrita por linha, modos ricos de abertura, rename/move/cópia, listagem de diretórios e biblioteca ampla de filesystem/texto.

103 - observação textual mínima em `--run`
- Décima nona fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `contem_verso(verso, verso) -> logica` para contenção textual mínima e `comeca_com(verso, verso) -> logica` para prefixo textual mínimo.
- Semântica operacional desta fase: ambas as intrínsecas operam apenas sobre `verso` e retornam `logica`, sem novos tipos e sem ampliar a API textual além de predicados simples.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `contem_verso` e `comeca_com` sem declaração explícita de função.
- Cobertura adicionada com testes automatizados de semântica/`--run` (casos positivos/negativos para cada intrínseca), integração com `ler_verso_arquivo(...)` e teste CLI com exemplo versionado (`examples/fase103_observacao_textual_minima_valido.pink`).
- Fora de escopo explícito nesta fase: `termina_com`, split/replace/regex/trim, busca por índice rica, parse textual amplo e biblioteca textual ampla.

104 - observação textual complementar mínima em `--run`
- Vigésima fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `termina_com(verso, verso) -> logica` para sufixo textual mínimo e `igual_verso(verso, verso) -> logica` para comparação textual explícita mínima.
- Semântica operacional desta fase: ambas as intrínsecas operam somente sobre `verso` e retornam `logica`, mantendo o subset textual pequeno sem novos tipos e sem redesign de runtime.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `termina_com` e `igual_verso` sem declaração explícita de função.
- Cobertura adicionada com testes automatizados de semântica/`--run` (casos positivos/negativos para cada intrínseca), integração com `ler_verso_arquivo(...)` e teste CLI com exemplo versionado (`examples/fase104_observacao_textual_complementar_minima_valido.pink`).
- Fora de escopo explícito nesta fase: split/replace/regex/trim, busca textual rica, parse textual amplo e biblioteca textual ampla.

105 - saneamento textual mínimo em `--run`
- Vigésima primeira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `vazio_verso(verso) -> logica` para detecção de vazio textual exato e `aparar_verso(verso) -> verso` para aparo mínimo de bordas.
- Semântica operacional desta fase: `vazio_verso` retorna `verdade` apenas para string vazia; `aparar_verso` remove whitespace de borda e preserva o miolo do texto.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram `vazio_verso` e `aparar_verso` sem declaração explícita de função.
- Cobertura adicionada com testes semânticos e de `--run` para casos positivos/negativos, caso onde `aparar_verso` resulta vazio e integração com `ler_verso_arquivo(...)`; CLI coberta por exemplo versionado (`examples/fase105_saneamento_textual_minimo_valido.pink`).
- Fora de escopo explícito nesta fase: split/replace/regex, variantes de trim separadas (`left/right`), normalização sofisticada, parse textual amplo e biblioteca textual ampla.

106 - normalização mínima de caixa em `--run`
- Vigésima segunda fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `minusculo_verso(verso) -> verso` e `maiusculo_verso(verso) -> verso`.
- Semântica operacional desta fase: conversão textual mínima de caixa no runtime usando comportamento padrão de `String` (sem casefolding e sem contrato locale-aware explícito).
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com testes semânticos e de `--run` para casos positivos, integração com `igual_verso`/`contem_verso`, integração com `ler_verso_arquivo(...)` e teste CLI com exemplo versionado (`examples/fase106_normalizacao_minima_caixa_valido.pink`).
- Fora de escopo explícito nesta fase: casefolding, locale-aware behavior, normalização Unicode sofisticada, split/replace/regex e biblioteca textual ampla.

107 - observação textual posicional mínima em `--run`
- Vigésima terceira fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `indice_verso_em(verso, verso) -> bombom` para primeira busca posicional mínima e `nao_vazio_verso(verso) -> logica` para ergonomia mínima de presença textual.
- Semântica operacional desta fase: `indice_verso_em` retorna o índice da primeira ocorrência do trecho e usa sentinela `18446744073709551615` (`u64::MAX`) quando o trecho está ausente; `nao_vazio_verso` retorna `verdade` apenas quando o texto possui comprimento maior que zero.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com testes semânticos e de `--run` para casos positivos/negativos (incluindo trecho ausente e string vazia), integração com `aparar_verso` + `ler_verso_arquivo(...)` e teste CLI com exemplo versionado (`examples/fase107_observacao_textual_posicional_minima_valido.pink`).
- Fora de escopo explícito nesta fase: última/múltiplas ocorrências, regex, split/replace/slicing geral, parse textual amplo e biblioteca textual ampla.

108 - append textual mínimo em `--run`
- Vigésima quarta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `abrir_anexo(verso) -> bombom` e `anexar_verso(bombom, verso) -> nulo`.
- Semântica operacional desta fase: `abrir_anexo` abre caminho textual em modo mínimo de append por handle; `anexar_verso` acrescenta texto no final do arquivo do handle sem newline implícito.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com testes semânticos e de `--run` (sucesso/falhas), integração com `criar_arquivo`, `ler_verso_arquivo` e `tamanho_arquivo`, além de teste CLI com exemplo versionado (`examples/fase108_append_textual_minimo_valido.pink`).
- Fora de escopo explícito nesta fase: append por caminho direto, múltiplos modos gerais de abertura, streaming, escrita por linha, seek/cursor público, encoding sofisticado e biblioteca ampla de filesystem/texto.

109 - leitura textual mínima direta por caminho em `--run`
- Vigésima quinta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `ler_arquivo_verso(verso) -> verso` e `arquivo_ou(verso, verso) -> verso`.
- Semântica operacional mínima: `ler_arquivo_verso` lê conteúdo textual completo diretamente por caminho e falha com diagnóstico claro quando não for possível ler; `arquivo_ou` tenta a mesma leitura mínima por caminho e retorna o `padrao` para ausência/impossibilidade simples de leitura no runtime atual.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com testes semânticos (reconhecimento, aridade e tipos), testes de `--run` (sucesso/fallback/falha de leitura por caminho) e teste CLI com exemplo versionado (`examples/fase109_leitura_textual_direta_por_caminho_valido.pink`).
- Fora de escopo explícito nesta fase: streaming, leitura incremental, escrita/append por caminho, modos ricos de abertura, seek/cursor público, encoding sofisticado, listagem de diretório e biblioteca ampla de filesystem/texto.

110 - entrada textual mínima em `--run`
- Vigésima sexta fase funcional do Bloco 8 — I/O e ecossistema útil.
- Superfícies escolhidas: `ouvir_verso() -> verso` e `ouvir_verso_ou(verso) -> verso`.
- Semântica operacional mínima: leitura textual mínima da stdin com retorno em `verso`, remoção mínima de newline final (`\n` e `\r\n`) e fallback textual simples em `ouvir_verso_ou` para EOF imediato/impossibilidade operacional simples de leitura.
- Pipeline alinhada no recorte: semântica/IR/CFG IR/selected/Machine/validações/runtime reconheceram as duas intrínsecas sem declaração explícita de função.
- Cobertura adicionada com testes semânticos (reconhecimento, aridade e tipo), testes de `--run`/CLI para sucesso e fallback em EOF, integração com `falar`, `aparar_verso`, `nao_vazio_verso`, `igual_verso` e exemplo versionado (`examples/fase110_entrada_textual_minima_valida.pink`).
- Fora de escopo explícito nesta fase: streaming, leitura por delimitador arbitrário, múltiplas linhas em lote, API rica de terminal, timeout, leitura não bloqueante, encoding sofisticado e biblioteca ampla de entrada textual.

111 - múltiplos blocos, labels e salto incondicional no backend nativo real
- Primeira fase funcional do Bloco 9 (item 9.1) concluída em recorte mínimo, auditável e sem abrir branch condicional.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar múltiplos blocos por função com labels e terminadores `jmp`/`ret`.
- Rejeições explícitas mantidas para branch condicional (`talvez/senao` e `sempre que`) no subset externo desta fase.
- Validações mínimas adicionadas para labels no caminho externo: bloco `entry` obrigatório, label duplicado inválido e `jmp` para label inexistente inválido.
- Exemplo versionado incluído: `examples/fase111_blocos_labels_salto_incondicional_valido.pink`.
- Base preparada para 9.2 sem antecipar `cmp`/`jcc`, loops, globais, `.rodata`, ABI mais larga ou tipos compostos.

112 - branch condicional real mínimo no backend nativo externo
- Segunda fase funcional do Bloco 9 (item 9.2) concluída em recorte mínimo, auditável e sem abrir loops.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar terminador condicional `br` com validação de alvos verdadeiro/falso e emissão `cmp $0` + `jne` no `.s`.
- Comparação mínima desta fase: `==` no corpo de bloco, com lowering direto para `cmp` + `sete` + `movzbq` antes do branch.
- Continuidade preservada do recorte 9.1: múltiplos blocos, labels, `jmp` incondicional, validação de `entry` e rejeição de label duplicado/`jmp` inexistente.
- Exemplo versionado incluído: `examples/fase112_branch_condicional_minimo_valido.pink`.
- Fora de escopo explícito preservado: loops (`sempre que`), comparações além de `==`, globais, `.rodata`, ABI mais larga, compostos e runtime nativa nova.

113 - loops reais mínimos no backend nativo externo
- Terceira fase funcional do Bloco 9 (item 9.3) concluída em recorte mínimo, auditável e sem puxar o item 9.4.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar loop mínimo real entre blocos no caminho de `sempre que`, com ciclo explícito por label de condição e `jmp` de retorno ao cabeçalho.
- Recorte de comparação do subset externo ampliado de forma mínima para `==` e `<` (com lowering auditável para `cmp` + `setcc` + `movzbq`), preservando as restrições de ABI e de superfície já consolidadas.
- Continuidade preservada do recorte 9.1/9.2: múltiplos blocos, labels, `jmp`, `br`, validação de `entry` e rejeição de label/alvo inexistente.
- Exemplo versionado incluído: `examples/fase113_loops_reais_minimos_validos.pink`.
- Fora de escopo explícito preservado: loops amplos, `break`/`continue` gerais, comparações além de `==`/`<`, globais, `.rodata`, ABI mais larga, compostos e runtime nativa nova.

114 - globais mínimas e base inicial de `.rodata` no backend nativo externo
- Quarta fase funcional do Bloco 9 (item 9.4) concluída em recorte mínimo, auditável e sem puxar o item 9.5.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar globais estáticas mínimas (`eterno`) de valor literal `bombom`/`logica`, com emissão auditável em `.section .rodata`.
- Leitura mínima de global estática no fluxo externo habilitada por load por símbolo (`movq simbolo(%rip), %reg`) no subset já suportado de instruções/terminadores.
- Validações mínimas adicionadas no caminho externo: símbolo global duplicado inválido, tipo global fora do recorte mínimo inválido e inicialização global não literal inválida.
- Exemplo versionado incluído: `examples/fase114_globais_minimas_rodata_base_valido.pink`.
- Fora de escopo explícito preservado: strings amplas, sistema global rico, arrays/structs globais, inicialização complexa, ABI mais larga, compostos e runtime nativa nova.

115 - ABI mínima mais larga (camada 1 conservadora) no backend nativo externo
- Quinta fase funcional do Bloco 9 (item 9.5, camada 1) concluída em recorte mínimo, auditável e sem puxar o item 9.6.
- Backend externo montável (`emit_external_toolchain_subset`) ampliou chamadas diretas de **até 2** para **até 3 argumentos `bombom`**, preservando frame mínimo e subset já consolidado.
- Convenção concreta desta camada no recorte Linux x86_64 hospedado: `%rdi` (arg0), `%rsi` (arg1), `%rdx` (arg2), `%rax` (retorno) e `%r10` (temporário volátil).
- Validações mínimas ampliadas no caminho externo: recusa explícita e auditável de função/call com **4+ argumentos**, mantendo as recusas de tipos não `bombom` e recursos fora do subset.
- Exemplo versionado incluído: `examples/fase115_abi_minima_mais_larga_camada1_valida.pink`.
- Limite explícito da fase: antes **até 2 argumentos `bombom`**; após a fase **até 3 argumentos `bombom`**.
- Fora de escopo explícito preservado: ABI ampla/plena, varargs, alinhamento geral sofisticado, múltiplos retornos, compostos por valor e item 9.6.

116 - compostos mínimos (camada 1 conservadora) no backend nativo externo
- Sexta fase funcional do Bloco 9 (item 9.6, camada 1) concluída em recorte mínimo, auditável e sem abrir compostos gerais.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar parâmetro `seta<bombom>` e `deref_load` mínimo (`*ptr`) em função externa, mantendo ABI por registrador sem composto por valor.
- Recorte estrutural explícito desta fase: par homogêneo de `bombom` em memória externa explícita, acessado por ponteiro e offsets definidos no código, sem layout geral automático.
- Validações mínimas ampliadas no caminho externo: recusa explícita para parâmetro fora do recorte (`seta<ninho>`, tipos gerais), recusa de `deref_load` fora de `bombom` e manutenção das recusas já existentes.
- Exemplo versionado incluído: `examples/fase116_compostos_minimos_camada1_valida.pink`.
- Fora de escopo explícito preservado: structs gerais, tuplas amplas, arrays gerais, compostos heterogêneos amplos, composto por valor em ABI, `deref_store` externo e layout/alinhamento geral sofisticado.

117 - compostos mínimos (camada 2 conservadora) no backend nativo externo
- Sétima fase funcional do Bloco 9 (item 9.6, camada 2) concluída em recorte mínimo, auditável e sem abrir compostos gerais.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar local `seta<bombom>` no mesmo recorte homogêneo da camada 1, permitindo materializar cursor com offset explícito e fazer dois `deref_load` auditáveis no fluxo real.
- Recorte estrutural explícito desta fase: dois loads homogêneos de `bombom` via `seta<bombom>` + offset explícito (`base + 8`) em função externa, sem composto por valor na ABI.
- Validações mínimas ampliadas no caminho externo: local fora de `bombom`/`seta<bombom>` continua recusado e caminho `fragil` em acesso indireto externo continua fora do subset.
- Exemplos versionados incluídos: `examples/fase117_compostos_minimos_camada2_valida.pink` e `examples/fase117_compostos_minimos_camada2_invalida.pink`.
- Fora de escopo explícito preservado: structs gerais, arrays gerais, compostos heterogêneos, composto por valor em ABI, retorno composto, `deref_store` externo, layout/alinhamento geral sofisticado e runtime nativa nova.

118 - compostos mínimos (camada 3 conservadora) no backend nativo externo
- Oitava fase funcional do Bloco 9 (item 9.6, camada 3) concluída em recorte mínimo, auditável e sem abrir compostos gerais.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar `deref_store` mínimo homogêneo (`*ptr = valor`) para `seta<bombom>` + `bombom`, preservando o recorte por ponteiro e sem composto por valor na ABI.
- Recorte estrutural explícito desta fase: leitura/escrita homogênea em `seta<bombom>` com `deref_load`/`deref_store` e offset explícito auditável no fluxo externo, sem sistema geral de campos/index/heterogeneidade.
- Validações mínimas ampliadas no caminho externo: `deref_store` continua restrito a `bombom`, caminho `fragil` em acesso indireto externo segue recusado e locals/parâmetros fora de `bombom`/`seta<bombom>` permanecem fora.
- Exemplos versionados incluídos: `examples/fase118_compostos_minimos_camada3_valida.pink` e `examples/fase118_compostos_minimos_camada3_invalida.pink`.
- Fora de escopo explícito preservado: structs gerais, arrays gerais, compostos heterogêneos, composto por valor em ABI, retorno composto, layout/alinhamento geral sofisticado e runtime nativa nova.

119 - compostos mínimos (camada 4 conservadora, fechamento do recorte homogêneo) no backend nativo externo
- Nona fase funcional do Bloco 9 (item 9.6, camada 4) concluída em recorte mínimo, auditável e sem abrir compostos gerais.
- Backend externo montável (`emit_external_toolchain_subset`) preservou parâmetros/locais `bombom` e `seta<bombom>` e consolidou a composição homogênea mínima em memória externa com sequência coesa de `deref_load` + `deref_store` + releitura, mantendo offsets explícitos auditáveis.
- Recorte estrutural explícito desta fase: unidade mínima de par homogêneo (`bombom` + `bombom`) manipulada por ponteiro (`seta<bombom>`) com leitura do primeiro, escrita no segundo, escrita de volta no primeiro e leitura final para resultado observável de composição, sem composto por valor na ABI.
- Validações mínimas preservadas no caminho externo: tipos fora de `bombom`/`seta<bombom>` seguem recusados, `fragil` em acesso indireto externo continua fora e recursos de composto amplo permanecem rejeitados.
- Exemplo versionado incluído: `examples/fase119_compostos_minimos_camada4_valida.pink`.
- Fora de escopo explícito preservado: structs gerais, arrays gerais, compostos heterogêneos, composto por valor em ABI, retorno composto, layout/alinhamento geral sofisticado e runtime nativa nova.

120 - tipos inteiros mais largos no backend nativo externo
- Primeira fase funcional do Bloco 10 (item 10.1) concluída em recorte mínimo, auditável e sem pacote amplo de tipos.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar `u32` em parâmetros e locais, preservando o recorte anterior (`bombom` e `seta<bombom>`) e sem abrir ABI plena.
- Operação mínima aberta nesta fase: movimentação/call no mesmo frame mínimo já existente para `u32`, com saída auditável no `.s` externo e fluxo real compilado/linkado/executado no exemplo da fase.
- Validações mínimas preservadas: tipos fora de `bombom`/`u32`/`seta<bombom>` continuam recusados no recorte externo desta fase.
- Exemplo versionado incluído: `examples/fase120_tipos_inteiros_mais_largos_valido.pink`.
- Fora de escopo explícito preservado: comparações ampliadas do item 10.2, casts amplos, ABI ampla/plena, família inteira de inteiros, compostos heterogêneos e redesign de pipeline.

121 - tipos inteiros mais largos (camada 2 conservadora) no backend nativo externo
- Segunda fase funcional do Bloco 10 (item 10.1) concluída em recorte pequeno, auditável e sem abrir pacote amplo de inteiros.
- Backend externo montável (`emit_external_toolchain_subset`) ampliou o mesmo eixo mínimo da Fase 120 para aceitar também `u64` em parâmetros e locais, preservando o subset anterior (`bombom`, `u32` e `seta<bombom>`).
- Operação aberta nesta fase: movimentação/call no mesmo frame mínimo já existente para `u64`, com emissão textual auditável no `.s` externo e fluxo real compilado/linkado/executado no exemplo da fase.
- Validações mínimas preservadas: tipos fora de `bombom`/`u32`/`u64`/`seta<bombom>` continuam recusados no recorte externo desta fase.
- Exemplo versionado incluído: `examples/fase121_tipos_inteiros_mais_largos_camada2_valido.pink`.
- Fora de escopo explícito preservado: comparações ampliadas do item 10.2, casts amplos, ABI ampla/plena, família inteira de inteiros, compostos heterogêneos e redesign de pipeline.

122 - comparações ampliadas (camada 1 conservadora) no backend nativo externo
- Terceira fase funcional do Bloco 10 (entrada do item 10.2) concluída em recorte pequeno, auditável e sem abrir universo amplo de comparações.
- Backend externo montável (`emit_external_toolchain_subset`) passou a aceitar comparação `!=` no mesmo fluxo externo já existente, preservando o recorte anterior (`==` e `<`) e mantendo semântica conservadora explícita.
- Recorte semântico desta fase: comparações mínimas suportadas no caminho externo ficam restritas a `==`, `!=` e `<`, sem ampliar para `<=`, `>`, `>=` ou pacote geral signed/unsigned.
- Emissão textual auditável aberta nesta fase: lowering de `CmpNe` com `cmpq` + `setne` + normalização booleana (`movzbq`) no `.s` externo.
- Exemplo versionado incluído: `examples/fase122_comparacoes_ampliadas_camada1_valido.pink`.
- Fora de escopo explícito preservado: comparações relacionais amplas, casts amplos, coerções implícitas gerais, ABI ampla/plena, item 10.3 (`quebrar`/`continuar`) e redesign de pipeline.

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

HF-3 - Bloco 8 (Fases 85–101): estabilização do `--run` (handles, I/O, caminho, texto)
- Pacote extraordinário de estabilização do Bloco 8 sem avançar trilha funcional.
- Bug #1 (interpreter.rs): uso de handle após `fechar` (use-after-close) produzia mensagem genérica "handle inválido", indistinguível de handle nunca aberto. Corrigido com rastreio de handles fechados (`closed_handles`) e mensagem específica "handle já fechado" em `ler_arquivo`, `ler_verso_arquivo`, `escrever`, `escrever_verso` e `fechar` (duplo). Classificador de erros atualizado com categoria `handle_ja_fechado` e dica diagnóstica.
- 11 testes novos adicionados cobrindo: uso de handle após `fechar` (4 intrínsecas), `fechar` duplo, leitura textual de arquivo vazio, leitura textual após escrita numérica (cross-type), `tamanho_arquivo` em diretório, `e_vazio` em diretório, `e_vazio` em caminho ausente, fluxo completo `criar_arquivo` → `escrever_verso` → `ler_verso_arquivo` → `fechar`.
- Cenários investigados sem bug reproduzível: `remover_arquivo` em diretório (já testado, erro OS claro), `remover_diretorio` em diretório não-vazio (já testado), `tamanho_arquivo`/`e_vazio` em caminho ausente (erro OS claro), `juntar_caminho` com strings vazias (semântica padrão de `PathBuf`), predicados de caminho em path inexistente (retornam `false` sem erro), validação semântica de tipos em todas as intrínsecas (correta).
- Nenhuma nova feature funcional adicionada.
- Suite completa pós-correção: todos os testes passam, `cargo clippy`/`fmt`/`doc` limpos.

HF-4 — varredura completa de repositório e higiene estrutural pós-B9
- Pacote extraordinário de hotfix transversal sem abertura de fase funcional nova.
- Correção estrutural do `history.md`: Fases 111–119 reencaixadas na seção FASES (antes eram listadas após DOCUMENTAÇÃO); Doc-20 reencaixada na seção DOCUMENTAÇÃO; ordem cronológica, categorias próprias e conteúdo histórico preservados.
- Mensagens de erro desatualizadas corrigidas em `backend_s.rs`: referências a "Fase 54" removidas de 3 diagnósticos do backend .s textual (retorno, slot e call), que permaneciam congeladas desde a fase original e ficaram enganosas após 65 fases de evolução.
- Texto de ajuda da CLI corrigido em `main.rs`: referência a "(Fase 54, ABI textual mínima)" removida do help de `--asm-s`; comentário de seção alinhado.
- Drift documental corrigido em `future.md`: "até 2 args" atualizado para "até 3 args" (alinhamento com Fase 115).
- Varredura completa de código, testes, exemplos e documentação sem introdução de feature funcional nova.
- Suite completa pós-correção: todos os testes passam, `cargo build`/`check`/`fmt --check`/`clippy`/`doc` limpos.

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

Doc-12 - sincronização ampla de `docs/future.md` com o estado real do projeto
- Rodada documental sem mudança funcional.
- `docs/future.md` revisado de ponta a ponta; itens implementados/parciais alinhados com `docs/history.md`, `docs/vocabulario.md`, `docs/agent_state.md` e README.
- Inconsistências corrigidas: `fragil` (Fases 52+72), `seta<T>` operações (Fases 65–68), `ninho` acesso operacional (Fase 69), arrays indexação operacional (Fase 70), `virar` lowering (Fase 71), `trazer` (Fase 60), geração x86_64 / ABI (Bloco 7, Fases 73–83), `sussurro` (Fase 56), `livre;` (Fase 57), linker script (Fase 58), `falar` (Fase 62), `verso` (Fase 61), `pink build` (Fase 63), inteiros signed runtime (Fase 64).
- Nota desatualizada de "HF-3" removida da entrada de inteiros signed; substituída por referência correta à Fase 64.
- Intro corrigido: "próxima trilha ativa" → "trilha ativa corrente" para o Bloco 7 (Fases 73–83 já entregues).
- Frentes prioritárias atualizadas para refletir status 🔶 atual de cada item.
- `docs/vocabulario.md`: `seta` movida de sugestões para keywords implementadas (implementado na Fase 48).

Doc-13 - limpeza editorial do README
- Rodada documental sem mudança funcional.
- Seção "Pipeline de backend textual" reescrita: substituição de todos os blocos "Estado explícito da Fase X" por descrição consolidada do estado atual e subset atual.
- Seções "O que o frontend faz hoje" e "O que não faz" corrigidas: referências a "nesta fase", "da fase" e "(Fase N)" substituídas por formulação de contrato presente ("atual", "hoje").
- Estrutura geral do README preservada; comandos, seção de documentação e tabela de fronteira auditável mantidos integralmente.
- Objetivo: README descreve o que existe hoje e como testar hoje; linguagem histórica/de-fase migrada para `history.md`.
- Rodada estritamente documental; sem alteração funcional de código, testes ou exemplos `.pink`.
- Objetivo: eliminar ambiguidade estratégica sobre o próximo grande rumo do projeto após o fechamento do Bloco 6.
- Bloco 6 — Memória operacional marcado como concluído no roadmap (Fases 64–72 entregues).
- **Bloco 7 — Backend nativo real** registrado formalmente como trilha ativa seguinte: transformar gradualmente o backend textual/experimental em backend nativo real mais utilizável.
- **Bloco 8 — I/O e ecossistema útil** registrado formalmente como bloco futuro já definido, não ativo: I/O, arquivo, verso operacional e base para tooling.
- Separação explícita preservada: trilha de soberania/backend (Bloco 7) vs. trilha de I/O/ecossistema (Bloco 8).
- Trilha ativa permanece única: apenas o Bloco 7 está marcado como próximo bloco ativo; o Bloco 8 aguarda consolidação suficiente do Bloco 7.
- Esta rodada não cria fase funcional; registra apenas a direção planejada.

Doc-14 - abertura documental do Bloco 8 e fechamento operacional do Bloco 7
- Rodada exclusivamente documental, sem nova fase funcional e sem implementação de `ouvir`.
- Transição registrada após consolidação suficiente do Bloco 7 (Fases 73–84), preservando que o bloco não está completo em sentido absoluto.
- `roadmap.md` atualizado: Bloco 7 deixou de ser trilha ativa e Bloco 8 passou a trilha ativa.
- `agent_state.md` e `handoff_codex.md` alinhados para o próximo passo funcional mínimo do Bloco 8: entrada básica com `ouvir` (ou equivalente) em `--run` para pelo menos um tipo básico.

Doc-15 - criação inicial de `manual.md` como manual de uso da linguagem
- Rodada exclusivamente documental, sem mudança funcional de código/runtime.
- `manual.md` criado na raiz do projeto com foco em uso real da Pinker no estado atual: estrutura básica, tipos, fluxo, funções, I/O, `verso`, exemplos completos e limites explícitos.
- Escopo preservado: manual orientado a uso, sem duplicar papel de `README.md`, `docs/history.md` ou `docs/roadmap.md`.
- `README.md` recebeu apenas ponteiro curto para `manual.md` na seção de ecossistema documental.
- `docs/agent_state.md`, `docs/handoff_codex.md` e `docs/phases.md` atualizados para refletir a nova peça documental e manter continuidade operacional.

Doc-16 - pacote paralelo de apoio (auditoria + corpus + mapeamento de codegen textual)
- Rodada de apoio paralela, sem abertura de fase funcional e sem implementação nova de backend/codegen.
- Drift factual corrigido no `manual.md`: limites de `verso` alinhados ao estado real (indexação mínima por `indice_verso` existe; seguem fora slicing/indexação negativa/formatação).
- `README.md` sincronizado com o corpus existente de backend externo (`--asm-s`) ao incluir comando explícito do exemplo versionado de recusa de `sempre que` (Fase 84).
- Corpus de uso real ampliado com exemplo pequeno e auditável de runtime/tooling: `examples/run_corpus_tooling_verso_minimo.pink` (`argumento_ou`, `tem_argumento`, `quantos_argumentos`, `falar` múltiplo e operações mínimas de `verso`).
- Cobertura de teste adicionada em `tests/interpreter_tests.rs` para o novo exemplo/corpus, sem abrir recurso novo.
- `docs/agent_state.md`, `docs/handoff_codex.md` e `docs/phases.md` atualizados para registrar a rodada documental/paralela e preservar continuidade histórica.

Doc-17 - alinhamento documental/operacional pós-Paralela-1 (binários + MCP)
- Rodada curta documental/operacional, sem abertura de fase funcional e sem expansão de linguagem/runtime/backend.
- `docs/doc_rules.md` atualizado para formalizar **rodadas paralelas de implementação** como categoria própria da crônica histórica em `docs/history.md` (seção `RODADAS PARALELAS`), distinta de Fase/HF/Doc.
- `docs/future.md` sincronizado com a precedência vigente (`roadmap`/`agent_state`/`handoff`): Bloco 8 permanece trilha ativa; Bloco 7 não é mais bloco ativo.
- Ambiguidade operacional de `cargo run` registrada e saneada: coexistência de binários (`pink`, `pinker_mcp`) exigiu explicitação de binário nos comandos de uso principal (`cargo run --bin pink -- ...`) e definição de `default-run = "pink"` para preservar ergonomia diária.
- README alinhado para remover padrões antigos ambíguos de auditoria/uso (ex.: `cargo run -- --check ...`, `cargo run -- --selected ...`) e para explicitar a existência/uso mínimo do binário `pinker_mcp`.
- Verificação prática do `pinker_mcp` executada no estado atual: transporte JSON-RPC 2.0 via stdio (JSON por linha), resposta positiva para `initialize`, `tools/list` e `tools/call` (`pinker_rodar`).


Doc-18 - reorganização estrutural da documentação (arquitetura dual Engine + Pinker/Rosa)
- Rodada exclusivamente documental, sem mudança funcional de linguagem/runtime.
- Arquivo mestre de navegação criado: `docs/atlas.md`.
- Arquivos estruturais novos: `docs/rosa.md` (canônico identitário) e `docs/ponte_engine_rosa.md` (ponte explícita factual ↔ visão).
- `docs/future.md` reposicionado como inventário técnico Engine de referência (não roadmap).
- `docs/parallel.md` reposicionado como acervo visionário de apoio (não backlog técnico).
- `docs/vocabulario.md` reestruturado para papel lexical maduro: critérios de keyword forte, sinais de keyword ruim, famílias lexicais, aceitas/rejeitadas/provisórias e distinção técnico/final/provisório.
- `README.md`, `docs/doc_rules.md`, `docs/agent_state.md`, `docs/handoff_codex.md` e `docs/phases.md` sincronizados com a nova arquitetura dual.
- Continuidade factual preservada: Fase atual 105, fase anterior 104, bloco ativo 8.

Doc-19 - encerramento formal do Bloco 8 e abertura canônica do Bloco 9
- Rodada exclusivamente documental, sem implementação funcional nova.
- Bloco 8 reconhecido como encerrado enquanto trilha ativa por suficiência funcional/documental no recorte entregue (Fases 85–110).
- Encerramento registrado como suficiência de trilha (não exaustão): futuras ampliações de I/O podem existir, mas deixam de ser frente principal ativa.
- Bloco 9 aberto como nova frente principal com tese explícita: ampliar cobertura semântica do backend nativo real já conquistado, sem reiniciar backend.
- Escada interna do Bloco 9 registrada em seis degraus auditáveis: 9.1 (blocos/labels/jump), 9.2 (branch condicional), 9.3 (loops), 9.4 (globais + base `.rodata`), 9.5 (ABI mínima mais larga), 9.6 (compostos mínimos).
- Exclusões explícitas registradas para conter scope creep (sem backend pleno, sem otimizador relevante, sem runtime grande, sem allocator completo, sem redesign amplo de pipeline, sem autohospedagem).
- Trava canônica de runtime nativa mínima registrada: runtime só entra para demonstrar capacidade semântica já conquistada no backend e não pode sequestrar o bloco.
- `agent_state.md`, `handoff_codex.md`, `phases.md`, `roadmap.md`, `ponte_engine_rosa.md`, `vocabulario.md` e `README.md` sincronizados com a transição B8 -> B9.

Doc-20 - encerramento conservador do Bloco 9
- Rodada exclusivamente documental, sem implementação funcional nova.
- Bloco 9 reconhecido como encerrado enquanto trilha ativa por suficiência conservadora, sem declarar exaustão total do espaço de backend nativo.
- Consolidação explícita do que o backend nativo externo realmente passou a suportar no bloco: 9.1 (múltiplos blocos/labels/`jmp`), 9.2 (branch condicional mínimo), 9.3 (loops mínimos), 9.4 (globais mínimas + base `.rodata`), 9.5 (ABI mínima mais larga em camada conservadora) e 9.6 (compostos mínimos).
- Item 9.6 reafirmado como fechado apenas no recorte homogêneo conservador atual (`seta<bombom>`, `deref_load` mínimo, offset explícito e `deref_store` mínimo homogêneo), sem heterogeneidade, sem composto por valor e sem sistema geral de agregados.
- Exclusões do bloco reforçadas para evitar superestimação do backend: sem backend pleno, sem ABI ampla/plena, sem composto por valor na ABI, sem retorno composto amplo, sem structs/arrays gerais e sem sistema geral de globais/layout sofisticado.
- Handoff operacional ajustado para impedir continuidade automática de 9.6 e para manter a próxima frente funcional em aberto disciplinado, sem reabertura do Bloco 9 por inércia.

Doc-21 - abertura canônica do Bloco 10
- Rodada exclusivamente documental, sem implementação funcional nova.
- Bloco 10 estabelecido como nova trilha ativa do roadmap após encerramento conservador do Bloco 9.
- Tese do bloco fixada como cobertura semântica do backend nativo real já existente, sem prometer backend pleno.
- Ordem interna refinada registrada como canônica: 10.1 tipos inteiros mais largos; 10.2 comparações ampliadas; 10.3 `quebrar`/`continuar`; 10.4 `ninho`/compostos heterogêneos mínimos; 10.5 `virar`; 10.6 `verso` mínima condicional.
- Decisão estrutural explicitada: `ninho` vem antes de `virar`.
- `verso` posicionada como item final e condicional, com trava de cautela para não sequestrar o bloco.
- Exclusões explícitas reforçadas para conter growth indevido (sem backend pleno, sem otimizador relevante, sem runtime grande, sem ABI ampla/plena, sem sistema geral de texto/compostos).


~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ RODADAS PARALELAS ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

Paralela-1 — negação bitwise dual (`~` + `nope`) + MCP mínimo
- Rodada paralela de implementação; não é hotfix, não é fase funcional, não é rodada documental pura.
- Não reordenou o roadmap nem conflitou com a trilha funcional ativa (Bloco 8).

Trilha A — negação bitwise unária dual:
- Adicionado `BitNot` ao pipeline completo: token (`Tilde`, `KwNope`), lexer, parser, AST, semântica, IR, ir_validate, cfg_ir_validate, instr_select, abstract_machine, abstract_machine_validate, interpreter, backend_text, backend_s.
- Forma simbólica `~` e forma textual Pinker `nope` produzem a mesma operação semântica (bitwise NOT).
- Ambas as superfícies reconhecidas como `UnaryOp::BitNot` desde o parser; sem distinção semântica posterior.
- Tipo aceito: qualquer inteiro já suportado (`bombom`, `u8`–`u64`, `i8`–`i64`); `logica` rejeitada na semântica.
- Testes adicionados: 6 casos em `tests/interpreter_tests.rs` cobrindo `~`, `nope`, equivalência, inversão de bits, dupla negação e tipo inválido.

Trilha B — MCP mínimo:
- Binário separado `pinker_mcp` criado em `src/bin/pinker_mcp.rs` (zero dependências externas).
- Transporte: JSON-RPC 2.0 via stdio (newline-delimited), sem LSP, sem Tree-sitter, sem servidor complexo.
- Ferramentas expostas: `pinker_checar`, `pinker_tokens`, `pinker_ast`, `pinker_ir` (modos: ir/cfg/selected/machine), `pinker_rodar`.
- Cada ferramenta despacha para a pipeline existente via biblioteca `pinker_v0`; sem reescrita de arquitetura.
- Limitação intencional: código inline apenas (sem resolução de imports entre módulos).
- Testes adicionados: 9 casos em `tests/mcp_tests.rs` cobrindo initialize, tools/list, checar, tokens, rodar, bitnot via MCP e erro de método desconhecido.
