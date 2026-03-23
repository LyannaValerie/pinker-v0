# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Doc-16 — pacote paralelo de apoio (auditoria + corpus + mapeamento de codegen textual)**.
- Rodada documental/paralela de baixo risco, sem disputa com trilha funcional ativa e sem abrir feature nova.

## 2. O que entrou na rodada atual
- Auditoria documental mínima com correção factual de drift em `manual.md` (limites de `verso` alinhados ao subset real já suportado).
- Sincronização leve do `README.md` com o corpus real de backend externo (`--asm-s`) via inclusão do comando do exemplo de recusa explícita de `sempre que` (Fase 84).
- Novo exemplo de corpus real em `examples/run_corpus_tooling_verso_minimo.pink`, combinando intrínsecas existentes (`argumento_ou`, `tem_argumento`, `quantos_argumentos`) com `falar` múltiplo e operações mínimas de `verso`.
- Teste de integração `--run`/CLI adicionado em `tests/interpreter_tests.rs` para validar o novo exemplo sem abrir funcionalidade nova.

## 3. Fora de escopo da rodada atual
- Implementação de lowering novo no codegen/backends (`--asm-s`, backend textual, backend externo).
- Abertura de recurso novo de runtime/CLI ou reestruturação arquitetural.
- Expansão do subset externo além dos limites atuais (controle de fluxo geral, 3+ parâmetros, tipos não `bombom`, memória indireta geral).

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **seguir refinamentos pequenos de tooling útil em `--run`, mantendo escopo mínimo e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **94**.
- Rodada documental mais recente: **Doc-16**.
- Hotfix extraordinário mais recente preservado: **HF-2 (Bloco 6, Fases 64–70)**.
- Hotfix histórico extraordinário preservado: **HF-1 (Fase 48-H1)**.

## 6. Precedência documental resumida
- Código mergeado prevalece sobre documentação.
- `roadmap.md` define trilha ativa.
- `future.md` organiza inventário técnico amplo.
- `parallel.md` mantém visão orientadora.
- `history.md` mantém histórico.
- `agent_state.md` mantém estado corrente.
- `handoff_codex.md` mantém handoff operacional curto.
- `doc_rules.md` mantém regras de documentação.
