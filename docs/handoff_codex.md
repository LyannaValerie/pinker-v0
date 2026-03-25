# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 114 — globais mínimas e base inicial de `.rodata` no backend nativo externo**.
- Quarta fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável agora aceita globais estáticas mínimas (`eterno` literal `bombom`/`logica`) no item 9.4 com emissão auditável em `.section .rodata`.
- Leitura mínima de global estática no fluxo externo entrou via load por símbolo (`movq simbolo(%rip), %reg`) no subset já suportado de instruções/terminadores.
- Validações mínimas no caminho externo foram ampliadas: símbolo global duplicado inválido, tipo global fora do recorte mínimo inválido e inicialização global não literal inválida.
- Exemplo versionado da fase adicionado (`examples/fase114_globais_minimas_rodata_base_valido.pink`) e cobertura de testes ampliada para emissão/uso observável de `.rodata`.

## 3. Continuidade preservada
- Fase funcional atual passa para **114**.
- Fase funcional anterior passa para **113**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Evoluir para o item **9.5** da escada do Bloco 9 (ABI mínima mais larga), mantendo recorte pequeno e auditável.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
