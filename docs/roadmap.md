# Roadmap macro da Pinker

## 1. Estado atual real do projeto

A Pinker v0 atual é um compilador/interpretador em camadas para uma linguagem pequena e tipada de forma estática, com pipeline textual e validadores entre estágios:

- frontend: lexer com spans, parser para `pacote`, `carinho`, `eterno`, `nova`/`mut`, `talvez/senao`, `sempre que`, `quebrar`, `continuar`, chamadas diretas por nome;
- tipos de usuário atuais: `bombom` (inteiro) e `logica` (booleano);
- semântica: validação de `principal`, retorno, aridade/tipos, mutabilidade e restrições de loop;
- lowering: AST -> IR estruturada -> CFG IR -> selected -> machine (pilha) -> pseudo-asm textual;
- execução: `--run` interpreta a Machine validada, com stack trace de runtime e limite preventivo de recursão;
- CLI: `--tokens`, `--ast`, `--json-ast`, `--ir`, `--cfg-ir`, `--selected`, `--machine`, `--pseudo-asm`, `--run`, `--check`;
- qualidade: suíte de testes ampla por camada e exemplos versionados para casos positivos/negativos.

Não há backend nativo real, FFI, ponteiros, structs, enums, sistema de módulos, stdlib de I/O ou build tool próprio neste estado.

## 2. O que a Pinker já consegue fazer

Hoje a Pinker já é madura para:

- validar e executar programas pequenos com inteiros/booleanos, if/else, loops, break/continue, recursão e chamadas entre funções;
- mostrar diagnósticos com contexto útil (incluindo source context em erros de parser/semântica e renderização dedicada para runtime);
- inspecionar cada camada intermediária de compilação de forma estável para auditoria (`--ir`, `--cfg-ir`, `--selected`, `--machine`, `--pseudo-asm`);
- manter continuidade técnica com validações internas (IR/CFG/selected/machine/backend textual).

Isso já sustenta fases curtas de evolução incremental sem refatorações grandes.

## 3. Limites atuais

Limites objetivos observáveis no repositório:

1. **Modelo de tipos muito pequeno**
   - sem strings, arrays, structs, enums, tipos numéricos de largura fixa e ponteiros.
2. **Sem sistema de módulos**
   - linguagem essencialmente monolítica por arquivo sem imports/visibilidade.
3. **Sem I/O e stdlib mínima**
   - não há operações de arquivo/entrada/saída como recursos de linguagem.
4. **Sem backend nativo**
   - a saída final é textual; execução depende do interpretador da Machine.
5. **Sem infraestrutura de build da própria linguagem**
   - não há compilação multi-arquivo/linkagem/tooling de projeto em Pinker.

## 4. Trilhas de evolução

### 4.1 Linguagem de uso geral

Blocos necessários para virar linguagem de uso geral (não de sistemas):

- ampliar tipos de dados de alto valor imediato:
  - strings úteis,
  - arrays/slices,
  - structs e enums,
  - inteiros com larguras explícitas (`u8/u16/u32/u64`, etc.).
- ampliar expressividade:
  - sistema de módulos/imports,
  - biblioteca padrão mínima (texto, coleções básicas, erros estruturados),
  - I/O básico.
- manter disciplina de validação por camada para não perder auditabilidade.

### 4.2 Linguagem de sistemas

Além do bloco de uso geral, programação de sistemas exige:

- ponteiros e operações de memória (deref, offset, casts controlados);
- layout explícito de dados (alinhamento/packed/union quando aplicável);
- recursos de baixo nível (`volatile`, `no-std`/freestanding, inline asm);
- backend nativo real + ABI + formato de objeto/binário + linkagem.

Sem backend nativo e sem memória explícita, Pinker ainda não entra de fato no domínio de sistemas.

### 4.3 Self-hosting

Para a Pinker compilar a própria Pinker no futuro, ainda faltam:

- módulos/imports reais e organização multi-arquivo;
- strings/coleções e manipulação de arquivos para front-end e tooling;
- modelo de erros estruturados em escala de compilador;
- base de stdlib + allocator;
- backend nativo e pipeline de build estável o suficiente para bootstrap.

### 4.4 Kernel / bare metal

Kernel/bare metal exige um subconjunto ainda mais específico:

- tipos pequenos e controle explícito de memória;
- controle de ABI/boot/linker script;
- freestanding e integração com código/asm de inicialização;
- emissor nativo confiável (no mínimo um target inicial).

Hoje esse objetivo é **distante** e depende primeiro de backend nativo + memória + tipos.

### 4.5 Tooling / build próprio

Para substituir Makefile/tooling de build, a Pinker precisa:

- módulos e compilação de múltiplas unidades;
- resolução de dependências e cache incremental;
- interface CLI de projeto (build/test/run);
- eventualmente uma linguagem de automação embutida ou bibliotecas de automação.

Sem esses blocos, a substituição de tooling externo ainda é prematura.

## 5. Dependências entre blocos

Ordem de dependência técnica recomendada:

1. **Tipos fundamentais + módulos**
   - desbloqueiam modelagem de programas maiores.
2. **Stdlib mínima + I/O + erros estruturados**
   - desbloqueiam utilidade prática fora de exemplos fechados.
3. **Backend nativo mínimo (um target)**
   - desbloqueia execução sem interpretador e aproxima sistemas.
4. **Memória explícita e recursos de baixo nível**
   - desbloqueiam programação de sistemas/kernels.
5. **Tooling/build próprio e bootstrap**
   - só faz sentido após 1–4 minimamente sólidos.

Paralelismo possível (com cuidado):

- design de vocabulário/keywords e desenho de módulos pode ocorrer em paralelo a melhorias de diagnósticos;
- testes de robustez por camada podem evoluir em paralelo a funcionalidades pequenas.

## 6. O que já está maduro para próximas fases curtas

Há espaço para fases curtas realistas **já agora** (sem inflar escopo):

1. limpeza documental de backlog (`docs/future.md`) para marcar itens já implementados (bitwise e `&&`/`||`);
2. reforço de exemplos e testes de regressão para cenários já suportados (mais casos negativos/diagnósticos);
3. pequenos refinamentos de UX de CLI/diagnóstico que não alterem semântica;
4. especificação incremental de módulos/imports em nível documental antes de implementação.

## 7. O que ainda é cedo demais

Para o estado atual, ainda é cedo/inflado tentar de imediato:

- self-hosting como meta de curto prazo;
- kernel/bare metal como meta de implementação próxima;
- full substituição de C/Assembly sem backend nativo e sem memória explícita;
- substituição de Python sem strings, coleções e stdlib de I/O madura;
- substituir Make/build tooling sem sistema de módulos, linkagem e gestão de projeto.

## 8. Próxima sequência recomendada

### Curto prazo (rodadas pequenas)

1. consolidar documentação de direção (este roadmap + alinhamento com `future.md`);
2. definir recortes mínimos de **módulos/imports** (somente design e critérios);
3. preparar base de tipos para expansão controlada (inteiros de largura fixa + tipo de string planejado) com estratégia de validação;
4. manter robustez diagnóstica e testes por camada.

### Médio prazo

1. implementar módulos/imports mínimos;
2. introduzir tipos de dados com maior retorno (strings/arrays/structs em escopo incremental);
3. iniciar stdlib mínima (I/O básico, erros estruturados);
4. projetar backend nativo mínimo com foco em correção antes de otimização.

### Longo prazo

1. backend nativo robusto + ABI/linkagem;
2. memória explícita e recursos de sistemas;
3. tooling/build próprio;
4. trilha de bootstrap para self-hosting.

## 9. Relação com docs/future.md

`docs/future.md` continua útil como catálogo amplo por camadas, mas precisa de curadoria contínua:

- contém itens já implementados no estado atual (ex.: bitwise, `&&`, `||`), que devem ser marcados para evitar falso backlog;
- mistura alguns itens de diferentes horizontes sem priorização prática de execução;
- deve continuar como inventário de possibilidades, enquanto este `roadmap.md` é o guia macro de priorização e dependências.

## 10. Critérios para revisar este roadmap no futuro

Revisar este roadmap quando ocorrer pelo menos um dos eventos:

1. entrada de novo bloco estrutural (ex.: módulos/imports, novos tipos centrais, backend nativo inicial);
2. mudança de horizonte do projeto (ex.: foco em uso geral vs. foco em sistemas);
3. fechamento de conjunto de fases curtas que altere dependências de médio prazo;
4. divergência detectada entre código real e documentação estratégica.

Checklist de revisão futura:

- confirmar estado real no código e testes;
- separar claramente concluído x planejado;
- manter curto/médio/longo prazo sem misturar escopos;
- evitar transformar visão distante em promessa imediata.
