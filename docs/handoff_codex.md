# Handoff Codex (executor)

## Rodada atual
- **Fase 36 implementada**: humanização das instruções individuais da saída `--machine` com comentários curtos por linha, sem alterar semântica.

## Estado real encontrado
- Continuidade histórica validada: 21a → 21b → 22 → 23a → 23b → 24 → 25 → 26 → 27a → 27b → 28a → 28b → 28c → 29 → 30 → 31 → 32 → 33 → 34 → 35.
- Workspace local mantido como fonte de verdade.
- Baseline antes das mudanças: `cargo build` e `cargo test` passavam.

## Ação aplicada (Fase 36)
- `src/abstract_machine.rs`:
  - `render_instr` agora mantém o opcode original e anexa comentário curto (`; ...`) explicando ação da instrução.
  - `render_term` agora mantém o terminador original e anexa comentário curto explicando o fluxo.
  - helper local `with_comment` adicionado para padronizar formatação.
- `tests/abstract_machine_tests.rs`:
  - testes de igualdade exata atualizados para o novo formato com comentários em linha.
  - cobertura nova adicionada para descrição de `call`, `br_true`, `jmp` e `ret`.
  - cobertura já existente de nomes limpos (`params/locals`) e linha `temps` mantida.
- Docs atualizados: `docs/phases.md`, `docs/agent_state.md`, `docs/handoff_codex.md`.

## O que melhorou na renderização `--machine`
- Leituras/escritas de slot ficaram explícitas (`carrega valor do slot`, `guarda topo da pilha`).
- Literais e globals indicam claramente empilhamento/carregamento.
- Chamadas (`call`/`call_void`) indicam retorno vs sem retorno.
- Operações aritméticas/lógicas/bitwise/comparações descrevem a intenção.
- Terminadores explicam desvio/retorno, especialmente `br_true` (“se topo for verdadeiro... senão...”).

## O que permaneceu igual
- Sem alteração na estrutura/semântica da Machine.
- Sem alteração em parser, semântica, lowering CFG, interpretador.
- Sem alteração em `--selected`, `--cfg-ir`, `--pseudo-asm`, `--run`.
- Sem criação de nova flag.

## Arquivos alterados
- `src/abstract_machine.rs`
- `tests/abstract_machine_tests.rs`
- `docs/phases.md`
- `docs/agent_state.md`
- `docs/handoff_codex.md`

## Comandos executados
- `cargo build`: ok
- `cargo check`: ok
- `cargo fmt --check`: ok
- `cargo test`: ok
- `cargo run -- --machine examples/showcase_completo.pink`: ok, com melhora visível nas linhas `vm`/`term`
