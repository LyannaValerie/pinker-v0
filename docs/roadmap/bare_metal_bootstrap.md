# Trilha bare-metal e bootstrap do Bloco 20

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

## Objetivo

Fechar a lacuna entre o backend nativo Linux já existente e uma cadeia Pinker freestanding capaz de produzir, inicializar e validar um kernel sem sistema hospedeiro.

Esta trilha não declara suporte bare-metal implementado. Ela organiza capacidades adultas, dependências, critérios de pronto e marcos verificáveis para que o Eixo A converja em artefatos de sistema operacional reais.

## Princípio anti-mínimo

Esta trilha obedece ao padrão pós-Eixo B definido em `docs/expandir.md`:

- nenhuma frente fecha como prova de conceito isolada;
- um arquivo `.o`, uma mensagem serial ou um boot único não bastam por si sós;
- cada fase deve entregar uma fatia vertical utilizável, com superfície, semântica, backend, diagnósticos, testes positivos e negativos, exemplo realista e documentação;
- limites continuam obrigatórios, mas não podem justificar esqueletos que precisem ser refeitos para o primeiro uso real;
- o caminho Linux existente deve permanecer compatível enquanto o caminho freestanding cresce;
- toda capacidade executável deve nascer com validação automatizada adequada ao domínio.

Anti-mínimo não significa implementar todo o sistema operacional em uma única fase. Significa que cada fase fecha um subproblema inteiro e operacional, em vez de apenas demonstrar que o subproblema existe.

## Posição no Bloco 20

A trilha é transversal e converge depois da Faixa 3 com as capacidades físicas da Faixa 7:

```text
Faixa 1 — abstrações de linguagem
  -> Eixo B — paridade nativa Linux
  -> Faixa 3 — ponteiros de função, alocador e inline assembly
  -> trilha bare-metal e bootstrap
       <-> Faixa 7 — memória, layout, ABI, atomics e fences
  -> Faixas 10–11 — kernel, concorrência, dispositivos e rede
```

Ela é uma ponte de toolchain, runtime e execução. Não substitui as faixas de linguagem e baixo nível.

## Estado já disponível

- backend próprio `.s` x86-64 System V;
- geração de ELF Linux por `pink build --nativo`;
- runtime nativo `pinker_rt` ligado por ABI C;
- ABI de funções, controle de fluxo, texto, coleções, leques e famílias sistêmicas no estado versionado;
- suíte de paridade interpretador × executável nativo para os casos compatíveis do Eixo B.

## Lacuna central

O caminho nativo atual pressupõe Linux e o runtime Rust/C ABI do workspace. Um SO exige target freestanding, artefatos relocáveis, ligação controlada, entrada própria, runtime autônomo, protocolo de boot, integração com hardware e validação reproduzível.

## Frentes adultas de entrega

### BM-A — toolchain freestanding completa

**Escopo:** transformar o backend atual em uma cadeia capaz de produzir e ligar artefatos sem Linux.

Deve cobrir em conjunto, ao longo das fases necessárias:

- target x86-64 freestanding explícito na CLI e no modelo de build;
- emissão de objeto relocável com símbolos globais/locais e relocations necessárias;
- referências entre unidades e diagnóstico de símbolo ausente ou incompatível;
- controle de `.text`, `.rodata`, `.data`, `.bss` e seções adicionais justificadas;
- linker script integrado ao fluxo de build, com endereço de carga e entry point configuráveis;
- mapa de símbolos e artefatos inspecionáveis;
- coexistência sem regressão com `pink build --nativo` para Linux;
- testes de objeto, ligação, erros e compatibilidade.

**Critério de fechamento da frente:** um projeto Pinker freestanding de múltiplos símbolos é compilado, ligado e inspecionado por fluxo oficial, sem depender de edição manual do assembly produzido.

### BM-B — bootstrap e runtime autônomo

**Escopo:** iniciar código Pinker e oferecer as fundações de execução sem `std`, libc ou syscalls Linux.

Deve cobrir:

- símbolo de entrada e convenção de passagem ao código Pinker;
- configuração de stack e inicialização determinística de `.bss`/`.data`;
- política explícita de abort/panic em ambiente freestanding;
- saída serial operacional e diagnosticável;
- alocador utilizável pela superfície Pinker e disciplina de liberação definida para o estágio;
- separação arquitetural entre `pinker_rt` hospedado e runtime freestanding;
- ausência verificável de dependências do sistema hospedeiro;
- testes de inicialização, erro, memória e regressão.

**Critério de fechamento da frente:** um programa Pinker inicializa repetidamente em ambiente freestanding, usa memória própria, comunica estado pela serial e encerra falhas de modo definido.

### BM-C — contrato de boot e fronteira de hardware

**Escopo:** tornar a imagem Pinker capaz de receber o estado da máquina e operar estruturas físicas com representação previsível.

Deve cobrir:

- protocolo de boot escolhido e documentado, sem acoplamento irreversível da linguagem a um bootloader;
- estruturas de informações de boot com layout, alinhamento e versionamento explícitos;
- mapa de memória e reserva das regiões ocupadas pelo kernel;
- acesso volátil/MMIO e raw I/O no domínio permitido;
- calling conventions e preservação de registradores para fronteiras especiais;
- tabelas e handlers de exceção/interrupção com caminho verificável;
- inline assembly real apenas onde a representação normal da Pinker não for suficiente;
- erros de compilação claros para layouts ou operações incompatíveis.

**Critério de fechamento da frente:** a imagem recebe e valida informações de boot, inicializa a fronteira de hardware definida e trata ao menos uma exceção/interrupção real sem depender de código de aplicação hospedado.

### BM-D — produto de build, emulação e gate de qualidade

**Escopo:** transformar as frentes anteriores em um produto reproduzível e continuamente verificável.

Deve cobrir:

- geração oficial de imagem de kernel, ISO ou disco conforme o protocolo adotado;
- manifesto de todos os artefatos e parâmetros de build;
- build reproduzível no ambiente suportado;
- execução automatizada em QEMU com timeout e captura serial;
- testes positivos de boot e testes negativos de imagem, linker, entry point e runtime;
- regressão conjunta do target Linux e do target freestanding;
- integração ao `make ci` e ao GitHub Actions quando a infraestrutura estiver disponível;
- artefatos de diagnóstico suficientes para investigar falha de boot.

**Critério de fechamento da frente:** a suíte oficial constrói a imagem a partir de um checkout limpo, inicia em QEMU, valida a saída esperada e falha de maneira útil diante de regressões.

## Critérios transversais por fase

Toda fase desta trilha deve registrar explicitamente:

1. **superfície:** comando, configuração e artefatos públicos utilizáveis;
2. **semântica:** regras, invariantes, compatibilidade e diagnósticos;
3. **backend/runtime:** implementação freestanding completa do subproblema e ausência de caminho fictício;
4. **testes:** casos positivos, negativos, regressão Linux e validação em emulador quando aplicável;
5. **exemplos:** uso composto que represente o marco real, não apenas chamada isolada;
6. **documentação:** manual, roadmap, handoff, histórico e inventários atualizados;
7. **evidência:** comandos e resultados objetivos que sustentem a declaração de pronto.

## Relação com a Faixa 7

A trilha bare-metal cria o caminho de artefato e execução. A Faixa 7 fornece as capacidades físicas necessárias para torná-lo correto:

- aritmética de ponteiros;
- structs empacotadas e layout controlado;
- bitfields;
- casts ponteiro ↔ inteiro;
- calling conventions;
- linker script e seções customizadas;
- operações atômicas;
- barriers e fences.

O item de linker script permanece na Faixa 7 como capacidade da linguagem/toolchain; BM-A define o contrato adulto de integração dessa capacidade ao produto freestanding.

## Marcos corrigidos

- **Marco SO 0 — boot freestanding verificável:** a toolchain oficial produz uma imagem reproduzível que inicia em QEMU, entra em código Pinker e comunica estado pela serial sem Linux.
- **Marco SO 1 — núcleo de baixo nível operacional:** a Pinker usa alocação própria, informações de boot, layout físico controlado e handler de interrupção real.
- **Marco SO 2 — kernel operacional:** scheduler, syscalls, sincronização essencial e isolamento de tarefas executam sobre o ambiente próprio.
- **Marco SO 3 — subsistema de dispositivos:** dispositivos de bloco e caractere sustentam armazenamento e console próprios; rede permanece uma expansão posterior da Faixa 11.

## Limites honestos

- nenhuma frente BM está implementada apenas por estar descrita aqui;
- o backend Linux atual continua sendo o único caminho nativo operacional até fases funcionais específicas;
- protocolo de boot, formato de imagem e decisões de ABI devem ser fechados com critérios técnicos nas fases correspondentes;
- esta trilha não substitui as Faixas 7, 10 ou 11;
- suporte multi-arquitetura permanece fora da primeira arquitetura freestanding;
- uma fase pode ter escopo delimitado, mas não pode fechar com stubs, placeholders ou uma demonstração que não sustente uso real;
- cada entrega exige fase numerada, lowering nativo, testes, exemplos, documentação e validação objetiva.

## Regra de execução

A ordem BM-A → BM-B → BM-C → BM-D expressa dependências dominantes, mas as frentes podem avançar de forma sobreposta quando uma fatia vertical exigir toolchain, runtime e teste juntos. Qualquer reordenação deve preservar o padrão anti-mínimo e não pode declarar marco alcançado sem artefato executável de ponta a ponta.
