# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 117 — compostos mínimos (camada 2 conservadora) no backend nativo externo**.
- Sétima fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável ampliou o recorte do item 9.6 com camada 2 conservadora: além do parâmetro `seta<bombom>`, agora aceita local `seta<bombom>` para materializar cursor com offset explícito mínimo e realizar dois `deref_load` homogêneos auditáveis.
- Recorte estrutural explícito da camada: dois loads homogêneos de `bombom` via `seta<bombom>` + `base + 8`, sem composto por valor na ABI.
- Validações mínimas no caminho externo ampliadas para manter honestidade de subset: local fora de `bombom`/`seta<bombom>` segue recusado e caminho `fragil` segue fora do subset externo.
- Exemplos versionados da fase adicionados (`examples/fase117_compostos_minimos_camada2_valida.pink` e `examples/fase117_compostos_minimos_camada2_invalida.pink`) com cobertura de testes para emissão auditável do novo degrau.

## 3. Continuidade preservada
- Fase funcional atual passa para **117**.
- Fase funcional anterior passa para **116**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Continuar o item **9.6** apenas se houver necessidade concreta e pequena para eventual fechamento conservador, sem abrir composto amplo/ABI composta.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
