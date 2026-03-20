# Handoff Codex (executor)

## Rodada atual
- Rodada funcional: **Fase 57 — freestanding / no-std (`livre;`)** (segundo item do Bloco 4).

## Convenção documental ativa
- Fase numerada (`Fase N`) = mudança funcional/estrutural real.
- Fase N-HX = rodada extraordinária de hotfixes (não avança funcionalidade).
- Rodada documental = ajuste de documentação/estratégia sem feature funcional.
- Rodada documental não recebe número de fase.

## O que foi atualizado

### Corretude (HIGH)
- **HF-1**: `Type::PartialEq` customizado em `src/ast.rs` — comparação estrutural ignora spans.
- **HF-2**: `PinkerError::Runtime` usa `Option<Span>` em `src/error.rs` — dummy span eliminado.
- **HF-3**: Runtime rejeita tipos signed (`i8`–`i64`) em `src/interpreter.rs` com erro explícito.
- **HF-4**: Validação de range de literais inteiros em `src/semantic.rs` (ex.: `300` em `u8` = erro).

### Manutenção (MEDIUM)
- **HF-5**: `main.rs` simplificado com macro `try_or_exit!` e booleanos de necessidade.
- **HF-6/7/8**: Decisões arquiteturais documentadas (bifurcação pipeline, else-if, KwSempre+KwQue).
- **HF-9**: CI alinhada com `rust-toolchain.toml` (1.78.0 em vez de `@stable`).
- **HF-10**: `clippy` adicionado ao CI; 4 warnings corrigidos.
- **HF-11**: `cargo doc --no-deps -D warnings` adicionado ao CI.

### Hygiene (LOW)
- **HF-15**: Mensagem de sucesso condicionada a nenhuma flag ativa.
- **HF-16**: `Cargo.toml` authors com `<>` correto.
- **HF-17**: `docs/future.md` atualizado para marcar itens já implementados.

## Decisões arquiteturais documentadas nesta rodada
- **Bifurcação pipeline (HF-6)**: `--pseudo-asm` parte de `selected_program`, `--run` parte de `machine_program`. Intencional — backend textual é representação alternativa da seleção; interpretador precisa da Machine validada.
- **Escopo else-if (HF-7)**: Assimetria é intencional — `senao talvez` é parsed como `senao { talvez ... }` aninhado, não como `else if` especial. Consistente com a gramática minimalista.
- **KwSempre + KwQue (HF-8)**: Duas keywords separadas por design — `sempre que` é combinação composicional, não keyword monolítica. Permite extensão futura (ex.: `sempre { }` para loop infinito).

## Estado operacional após a rodada
- Continuidade histórica preservada (Fase 48 funcional → Fase 48-H1 hotfixes).
- Roadmap principal inalterado; Bloco 3 permanece como trilha funcional ativa.
- CI agora inclui clippy e doc validation além de build/check/fmt/test.
- Runtime signed bloqueado explicitamente até implementação correta de representação signed.


## O que entrou na Fase 49
- Frontend: parser agora suporta cadeia postfix (`call` + `obj.campo` + `arr[idx]`) com precedência preservada.
- AST/JSON/printer: novos nós de expressão para acesso a campo e indexação.
- Semântica: validação de acesso a campo apenas em base `ninho` e indexação apenas em array fixo com índice inteiro.
- IR estruturada: representação mínima para campo/index (`ValueIR::FieldAccess` e `ValueIR::Index`).
- Decisão deliberada de escopo: leitura apenas; escrita em LHS não adicionada.
- Downstream deliberadamente limitado: CFG/execução ainda retornam erro claro para esses nós nesta fase, evitando redesign de memória/runtime.
- Exemplos versionados adicionados para `--check` (casos positivo/negativo).

## Fora de escopo mantido
- dereferência/aritmética de ponteiros e acesso via `seta<T>`
- `sizeof`/alinhamento e `volatile`
- backend nativo e modelagem de layout físico
- bounds-check de runtime/estático sofisticado

## O que entrou na Fase 50
- Frontend: keyword `virar` adicionada e parse de cast explícito como expressão local (`expr virar tipo`).
- AST/JSON/printer: novo nó de expressão `Cast`.
- Semântica: política mínima e explícita de cast permitido apenas para inteiro->inteiro (`bombom`, `u8/u16/u32/u64`, `i8/i16/i32/i64`), incluindo alias resolvido ao tipo subjacente.
- Semântica: casts envolvendo `logica`, ponteiros (`seta`), `ninho` e arrays fixos permanecem fora de escopo, com erro semântico explícito.
- IR estruturada: representação mínima (`ValueIR::Cast`) e validação (`ir_validate`) coerente com a mesma política inteiro->inteiro.
- Downstream deliberadamente limitado: CFG/execução ainda retornam erro claro para cast nesta fase, evitando redesign de runtime/memória.
- Continuidade preservada: Fase 48-H1 segue sendo rodada extraordinária/hotfix sem reordenar o roadmap principal.

## Próximo item normal do roadmap principal
- Bloco 3, item 1: backend textual `.s`.

## O que entrou na Fase 51
- Frontend: keywords `peso` e `alinhamento` adicionadas no lexer/token e parse de `peso(tipo)`/`alinhamento(tipo)` como expressões explícitas.
- AST/JSON/printer: novos nós de expressão para consulta de tamanho e alinhamento por tipo.
- Semântica: cálculo estático de layout/alinhamento com política mínima explícita e determinística para escalares, `seta<T>`, arrays fixos, `ninho` e aliases via tipo subjacente.
- Política desta fase:
  - `bombom` equivale a `u64` para layout (`8/8`);
  - `logica` usa `1/1`;
  - `seta<T>` usa `8/8` abstrato fixo;
  - `[T; N]` usa `N * peso(T)` e alinhamento de `T`;
  - `ninho` usa alinhamento natural por campo + alinhamento máximo da struct + arredondamento final do tamanho.
- IR estruturada: lowering converte `peso`/`alinhamento` em literal inteiro constante (`bombom`) e mantém pipeline downstream sem runtime novo.
- Continuidade preservada: Fase 48-H1 segue sendo rodada extraordinária/hotfix sem reordenar o roadmap principal.

## Fora de escopo mantido
- `volatile`
- dereferência real e aritmética de ponteiros
- ABI/layout físico final orientado a backend
- backend nativo e redesign de runtime

## O que entrou na Fase 52
- Frontend: keyword `fragil` adicionada e parse mínimo de qualificador de tipo `fragil seta<T>`.
- AST/JSON/printer: tipo ponteiro agora preserva qualificação `is_volatile`, com render textual explícito (`fragil seta<...>`).
- Semântica: `fragil` é aceito apenas quando qualifica `seta<T>`; usos fora desse formato são rejeitados com diagnóstico claro.
- IR estruturada: `TypeIR::Pointer` preserva o bit `is_volatile`; render da IR passa a exibir `fragil seta<?>` quando aplicável.
- Política operacional desta fase: `fragil` é somente marca semântica propagada no pipeline (sem dereferência real, sem MMIO, sem fences, sem backend nativo).
- Exemplos versionados e cobertura de testes adicionados para caso positivo e negativo com `--check`.
- Continuidade preservada: Fase 48-H1 segue como rodada extraordinária/hotfix sem reordenar o roadmap principal.

## Fora de escopo mantido
- dereferência real e aritmética de ponteiros
- MMIO/hardware real e semântica de ordenação/barreiras
- backend nativo/ABI e lowering operacional de memória para `volatile`

## Rodada documental paralela (sem número de fase)
- Executada em paralelo à Fase 51 por agente separado; sem conflito com este handoff.
- Alterações exclusivamente documentais: `handoff_auditor.md` abandonado, `future.md` normalizado, `phases.md` e `agent_state.md` atualizados.
- Nenhuma alteração funcional de parser, semântica, IR, CFG, Machine ou runtime.
- Próximo item funcional do roadmap agora é: Bloco 3, item 1 (backend textual `.s`).


## O que entrou na Fase 53
- CLI: nova flag `--asm-s` (aliases `--asm` e `--s`) para emissão textual `.s` separada de `--pseudo-asm`.
- Fonte da emissão `.s`: camada `selected` (com validação de seleção preservada), sem depender da Machine e sem executar interpretador.
- Backend textual `.s`: formato estável assembly-like com labels por função/bloco, diretivas textuais simples (`.text`, `.globl`, `.section .rodata`) e instruções derivadas do subset atual (`mov`, unárias/binárias, `call`, `jmp`, `br`, `ret`).
- Política de subset explícita: suporta apenas tipos escalares (`bombom`, `u8..u64`, `i8..i64`, `logica`, `nulo`) e falha claramente para tipos ainda fora de escopo nesta fase (`seta`, `ninho`, arrays fixos).
- `--pseudo-asm` foi preservado intacto para auditoria da camada textual anterior.

## O que entrou na Fase 54
- `--asm-s` permaneceu derivado da camada `selected` (sem passar por Machine) e ganhou contrato de ABI textual mínima interna (`pinker.text.v0`).
- Convenção mínima explicitada por função:
  - símbolo exportado (`.globl <nome>`);
  - parâmetros posicionais `@argN` mapeados para slots nomeados (`$param`);
  - retorno textual por `@ret` (ou `ret_void` para funções `nulo`);
  - marcações estruturais de prólogo/epílogo (`.L<func>_prologue` / `.L<func>_epilogue`) sem stack frame real.
- Chamadas passaram a exibir convenção posicional de argumentos no comentário (`abi.call [@argN=...]`) e destino de retorno textual.
- Subset mantido: apenas escalares (`bombom`, `u8..u64`, `i8..i64`, `logica`, `nulo`); tipos fora de escopo (`seta`, `ninho`, arrays) seguem com erro explícito.
- Escopo deliberadamente preservado: sem assembler/linker, sem ABI final de plataforma, sem register allocation e sem executável real.

## Continuidade de roadmap após Fase 54
- Fase 53 permanece a fase funcional principal anterior.
- Fase 48-H1 permanece rodada extraordinária/hotfix anterior, sem reordenar a trilha principal.
- Bloco 2 foi encerrado em Fase 52 e o Bloco 3 segue ativo (itens 1 e 2 concluídos).
- Próximo item normal do roadmap principal: Bloco 3, item 3 (uso de assembler/linker externo).

## O que entrou na Fase 55
- Emissão `.s` textual da Fase 54 foi preservada intacta (`--asm-s` continua derivado da camada `selected`).
- Backend `.s` ganhou um emissor adicional de **subset externo mínimo**:
  - sem globais;
  - função única `principal() -> bombom`;
  - retorno inteiro constante em bloco único (`mimo <constante>;`).
- Para prova real de integração, o emissor externo mapeia `principal` para símbolo `main` e gera assembly mínimo Linux x86_64 (`movq $const, %rax; ret`).
- Teste de integração condicional adicionado:
  - detecta driver C disponível (`cc`/`gcc`/`clang`);
  - compila e linka o `.s` gerado;
  - executa binário e valida `exit code` esperado.
- Estabilidade preservada: se ambiente não for Linux x86_64 ou não tiver driver C, o teste de fluxo real retorna cedo (skip lógico), sem quebrar suíte padrão.

## Continuidade de roadmap após Fase 55
- Fase 54 passa a ser a fase funcional principal anterior.
- Fase 48-H1 permanece rodada extraordinária/hotfix anterior, sem reordenar a trilha principal.
- Bloco 3 foi encerrado com os itens 1, 2 e 3 concluídos.
- Próximo item normal do roadmap principal: Bloco 4, item 1 (inline asm).

## O que entrou na Fase 56
- Frontend: keyword `sussurro` adicionada com parse de statement mínimo `sussurro("...");` e variante com múltiplas strings literais.
- AST/JSON/printer: novo nó explícito `InlineAsmStmt { chunks }` preservando conteúdo textual opaco.
- Semântica: validação mínima de uso (ao menos uma string literal e rejeição de string vazia).
- IR estruturada: instrução explícita `InlineAsm` preservada na renderização de `--ir`.
- Decisão de escopo deliberada: inline asm é apenas statement nesta fase (não expressão).
- Downstream deliberadamente limitado: CFG IR ainda não lowera `InlineAsm` e retorna erro claro ao tentar `--cfg-ir`; runtime/`--run` seguem sem execução de inline asm.
- Exemplos versionados: `check_inline_asm_valido`, `check_inline_asm_multilinha`, `check_inline_asm_invalido_vazio`.
- Continuidade preservada: Fase 48-H1 segue como rodada extraordinária/hotfix sem reordenar o roadmap principal.

## Continuidade de roadmap após Fase 56
- Fase 55 passa a ser a fase funcional principal anterior.
- Fase 48-H1 permanece rodada extraordinária/hotfix anterior, sem reordenar a trilha principal.
- Bloco 4 foi iniciado com o item 1 concluído.
- Próximo item normal do roadmap principal: Bloco 4, item 2 (freestanding / no-std).

## O que entrou na Fase 57
- Frontend: keyword `livre` adicionada com parse de marcador de unidade `livre;`.
- Regra de posição: marcador permitido somente no topo do programa, no máximo uma vez, após `pacote` e antes dos itens.
- AST/JSON/printer: `Program` preserva a marca de freestanding como metadata explícita.
- Semântica/política: `livre;` expressa intenção de ambiente freestanding/no-std, sem efeito de boot/runtime real nesta fase.
- Propagação no pipeline: IR, CFG, selected, pseudo-asm e backend `.s` passaram a renderizar `mode livre|hospedado`.
- Relação com `principal`: exigência de `principal() -> bombom` foi mantida para preservar continuidade.
- Relação com `sussurro`: inline asm permanece permitido em código marcado como `livre`, sem semântica operacional adicional.
- Exemplos versionados: `examples/check_freestanding_valido.pink` e `examples/check_freestanding_invalido_fora_topo.pink`.

## Continuidade de roadmap após Fase 57
- Fase 56 passa a ser a fase funcional principal anterior.
- Fase 48-H1 permanece rodada extraordinária/hotfix anterior, sem reordenar a trilha principal.
- Bloco 4 avançou com os itens 1 e 2 concluídos.
- Próximo item normal do roadmap principal: Bloco 4, item 3 (linker script / boot entry).
