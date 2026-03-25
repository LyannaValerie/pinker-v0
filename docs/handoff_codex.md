# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Doc-20 — encerramento conservador do Bloco 9**.
- Rodada documental, sem implementação funcional nova.

## 2. Consolidação factual do Bloco 9
- O Bloco 9 foi encerrado como trilha ativa por suficiência conservadora: ampliou de modo real e auditável o backend nativo externo, sem transformá-lo em backend pleno.
- Recorte entregue no backend externo consolidado em seis degraus: 9.1 (múltiplos blocos/labels/`jmp`), 9.2 (branch condicional mínimo), 9.3 (loops mínimos), 9.4 (globais mínimas com base `.rodata`), 9.5 (ABI mínima mais larga, ainda conservadora) e 9.6 (compostos mínimos).
- Item 9.6 está fechado apenas no recorte homogêneo conservador atual: `seta<bombom>`, `deref_load` mínimo, offset explícito e `deref_store` mínimo homogêneo; sem heterogeneidade, sem composto por valor na ABI e sem sistema geral de agregados.

## 3. Continuidade preservada
- Fase funcional atual permanece **119**.
- Fase funcional anterior permanece **118**.
- Bloco 9 deixa de ser trilha principal ativa.
- Encerramento do Bloco 9 não proíbe evolução futura de backend externo; apenas remove continuidade automática desta trilha.

## 4. Restrições operacionais explícitas
- Não continuar automaticamente o item 9.6.
- Não reabrir o Bloco 9 salvo necessidade extraordinária, pequena e bem justificada.
- Manter exclusões explícitas: sem backend pleno, sem ABI ampla/plena, sem composto por valor na ABI, sem retorno composto amplo, sem structs/arrays gerais e sem sistema global/layout geral sofisticado.
- A próxima frente funcional deve ser definida conscientemente, não por inércia documental.

## 5. Precedência resumida
- Código mergeado prevalece.
- `roadmap.md` define a ordem ativa.
- `history.md` guarda continuidade factual.
- `atlas.md` organiza navegação dual.
- `rosa.md`/`vocabulario.md` guardam identidade lexical e visão.
