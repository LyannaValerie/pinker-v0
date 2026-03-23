# Handoff Codex (operacional curto)

## 1. Rodada atual
- **Fase 95 — ambiente mínimo de processo em `--run` (fallback de env + diretório atual)**.
- Rodada funcional pequena e auditável na trilha ativa do Bloco 8, sem abrir biblioteca ampla de tooling.

## 2. O que entrou na rodada atual
- Intrínseca `ambiente_ou(verso, verso) -> verso` no `--run` para leitura mínima de variável de ambiente com fallback.
- Intrínseca `diretorio_atual() -> verso` no `--run` para leitura mínima do diretório corrente.
- Alinhamento do pipeline (semântica/IR/CFG IR/selected/Machine/validações/runtime) para reconhecer as intrínsecas sem declaração explícita.
- Novos exemplos versionados da fase: `fase95_ambiente_processo_minimo_valido`, `fase95_diretorio_atual_minimo_valido` e `fase95_argumento_ou_ambiente_ou_valido`.
- Cobertura de testes ampliada em semântica e CLI/runtime para fallback de ambiente, leitura de ambiente real, diretório atual e integração com `argumento_ou`.

## 3. Fora de escopo da rodada atual
- Mutação/listagem ampla de variáveis de ambiente.
- Mudança de diretório, listagem de diretórios e API ampla de paths.
- Processos externos e qualquer expansão de parser de CLI/subcomandos.

## 4. Próximo item normal
- Trilha ativa permanece: **Bloco 8 — I/O e ecossistema útil**.
- Próximo item funcional sugerido: **seguir refinamentos pequenos de tooling útil em `--run`, mantendo escopo mínimo e auditável**.

## 5. Observações operacionais curtas
- Última fase funcional concluída: **95**.
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
