# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **HF-4 — varredura completa de repositório e higiene estrutural pós-B9**.
- Hotfix transversal sem implementação funcional nova.

## 2. Escopo do HF-4
- Correção estrutural de `history.md`: Fases 111–119 reencaixadas na seção FASES; Doc-20 reencaixada na seção DOCUMENTAÇÃO.
- Mensagens de erro desatualizadas corrigidas em `backend_s.rs` (referências a "Fase 54" removidas de diagnósticos).
- Texto de ajuda da CLI corrigido em `main.rs` (referência a "Fase 54" removida de `--asm-s`).
- Drift documental corrigido em `future.md` ("até 2 args" → "até 3 args").
- Varredura completa de código, testes, exemplos e documentação.
- Nenhuma feature funcional nova introduzida.

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
