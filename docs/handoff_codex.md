# Handoff Codex (executor)

## Rodada atual
- Rodada funcional: **Fase 52 — `volatile` (`fragil`)** (quinto item do Bloco 2).

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
- Roadmap principal inalterado; Bloco 2 continua na próxima fase funcional.
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
