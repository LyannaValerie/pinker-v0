# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 119 — compostos mínimos (camada 4 conservadora, fechamento do recorte homogêneo) no backend nativo externo**.
- Nona fase funcional do Bloco 9 concluída em recorte mínimo, auditável e conservador.

## 2. O que entrou na rodada atual
- Backend externo montável consolidou o recorte do item 9.6 em camada 4 conservadora: manteve `seta<bombom>`, `deref_load` e `deref_store` homogêneos e fechou o uso como par mínimo auditável com sequência coesa de leituras/escritas e releitura por offsets explícitos.
- Recorte estrutural explícito da camada: unidade mínima homogênea externa (par de `bombom` em memória apontada) com manipulação observável mais forte no output (`--asm-s`) sem composto por valor na ABI.
- Validações mínimas preservadas no caminho externo: usos fora de `bombom`/`seta<bombom>` seguem recusados; caminho `fragil` em acesso indireto externo continua fora; composto amplo/heterogêneo continua fora.
- Exemplo versionado da fase adicionado (`examples/fase119_compostos_minimos_camada4_valida.pink`) com cobertura de testes para emissão auditável e fluxo real do novo fechamento conservador.

## 3. Continuidade preservada
- Fase funcional atual passa para **119**.
- Fase funcional anterior passa para **118**.
- Bloco ativo permanece **Bloco 9 — ampliação do backend nativo real**.
- Bloco 8 permanece fechado como trilha ativa; pode receber ampliações futuras apenas de forma subordinada e extraordinária.

## 4. Próximo item normal
- Item **9.6** encontra-se fechado no recorte homogêneo conservador atual; qualquer nova evolução deve ser excepcional, pequena e explícita, sem abrir composto amplo/ABI composta.
- Não reabrir Bloco 8 salvo necessidade extraordinária, objetiva e bem justificada.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define trilha ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
