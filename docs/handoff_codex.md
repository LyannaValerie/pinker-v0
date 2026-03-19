# Handoff Codex (executor)

## Rodada atual
- Rodada **extraordinária de hotfixes**: Fase 48-H1 do histórico interno, corrigindo problemas de corretude, CI e manutenção acumulados até a Fase 48.

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
