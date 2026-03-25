# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 112 — branch condicional real mínimo no backend nativo externo**.
- Segunda fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável agora aceita branch condicional mínimo no terminador (`br`) entre dois alvos válidos (Fase 112, item 9.2).
- Recorte de comparação desta fase: apenas `==` no corpo da função, com emissão auditável no `.s` (`cmp` + `sete`/`movzbq`) e desvio condicional (`cmp $0` + `jne`).
- Validação mínima no caminho externo estendida para `br`: alvo verdadeiro inexistente inválido e alvo falso inexistente inválido; validações de `entry`, label duplicado e `jmp` inexistente permanecem.
- Exemplo versionado da fase adicionado (`examples/fase112_branch_condicional_minimo_valido.pink`) e cobertura de testes ampliada.

## 3. Continuidade preservada
- Fase funcional atual passa para **112**.
- Fase funcional anterior passa para **111**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Evoluir para o item **9.3** da escada do Bloco 9 (loops reais mínimos), mantendo recorte pequeno e auditável.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
