# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 113 — loops reais mínimos no backend nativo externo**.
- Terceira fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável agora aceita loop mínimo real no caminho de `sempre que` via retorno de salto ao cabeçalho entre blocos válidos (Fase 113, item 9.3).
- Recorte de comparação do backend externo foi ampliado de forma mínima para `==` e `<`, mantendo lowering auditável (`cmp` + `setcc` + `movzbq`) e branch por `cmp $0` + `jne`.
- Validações mínimas no caminho externo preservadas e alinhadas: `entry` obrigatório, label duplicado inválido, alvo inexistente em `jmp`/`br` inválido.
- Exemplo versionado da fase adicionado (`examples/fase113_loops_reais_minimos_validos.pink`) e cobertura de testes ampliada para fluxo real com ciclo.

## 3. Continuidade preservada
- Fase funcional atual passa para **113**.
- Fase funcional anterior passa para **112**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Evoluir para o item **9.4** da escada do Bloco 9 (globais mínimas + base `.rodata`), mantendo recorte pequeno e auditável.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
