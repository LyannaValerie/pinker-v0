# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 111 — múltiplos blocos, labels e salto incondicional no backend nativo real**.
- Primeira fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável agora aceita múltiplos blocos por função com labels e terminadores `jmp`/`ret` (Fase 111, item 9.1).
- Rejeição explícita de branch condicional foi mantida no subset externo (`talvez/senao` e `sempre que`).
- Validação mínima de labels adicionada no caminho externo: `entry` obrigatório, label duplicado inválido e `jmp` para alvo inexistente inválido.
- Exemplo versionado da fase adicionado (`examples/fase111_blocos_labels_salto_incondicional_valido.pink`) e cobertura de testes ampliada.

## 3. Continuidade preservada
- Fase funcional atual passa para **111**.
- Fase funcional anterior passa para **110**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Evoluir para o item **9.2** da escada do Bloco 9 (branch condicional real), mantendo recorte pequeno e auditável.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
