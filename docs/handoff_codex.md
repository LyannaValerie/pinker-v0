# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 116 — compostos mínimos (camada 1 conservadora) no backend nativo externo**.
- Sexta fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável abriu o primeiro recorte de composto mínimo no item 9.6: parâmetro `seta<bombom>` + `deref_load` (`*ptr`) para leitura auditável em função nativa externa.
- Recorte estrutural explícito da camada: par homogêneo de `bombom` em memória externa explícita, sem composto por valor na ABI.
- Validações mínimas no caminho externo ampliadas para manter honestidade de subset: recusa de parâmetro fora de `bombom`/`seta<bombom>`, recusa de `deref_load` fora de `bombom` e manutenção das recusas já existentes.
- Exemplo versionado da fase adicionado (`examples/fase116_compostos_minimos_camada1_valida.pink`) e cobertura de testes ampliada para emissão auditável do acesso indireto mínimo.

## 3. Continuidade preservada
- Fase funcional atual passa para **116**.
- Fase funcional anterior passa para **115**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Continuar o item **9.6** em camada conservadora adicional, somente se houver necessidade concreta e pequena.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
