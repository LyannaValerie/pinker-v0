# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 118 — compostos mínimos (camada 3 conservadora) no backend nativo externo**.
- Oitava fase funcional do Bloco 9 concluída em recorte mínimo e auditável.

## 2. O que entrou na rodada atual
- Backend externo montável ampliou o recorte do item 9.6 com camada 3 conservadora: além de parâmetros/locais `seta<bombom>` e `deref_load`, agora aceita `deref_store` mínimo homogêneo (`*ptr = valor`) no caminho externo.
- Recorte estrutural explícito da camada: leitura e escrita homogêneas de `bombom` via `seta<bombom>` com offset explícito auditável, sem composto por valor na ABI.
- Validações mínimas no caminho externo ampliadas para manter honestidade de subset: `deref_store` fora de `bombom` segue recusado, `fragil` em acesso indireto externo segue fora e locals/parâmetros fora de `bombom`/`seta<bombom>` continuam recusados.
- Exemplos versionados da fase adicionados (`examples/fase118_compostos_minimos_camada3_valida.pink` e `examples/fase118_compostos_minimos_camada3_invalida.pink`) com cobertura de testes para emissão auditável do novo degrau.

## 3. Continuidade preservada
- Fase funcional atual passa para **118**.
- Fase funcional anterior passa para **117**.
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
