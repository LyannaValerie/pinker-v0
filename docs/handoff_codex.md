# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 158 — formatação e dados estruturados: CSV mínimo (camada 1 conservadora)**.
- **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase funcional ativa passa a ser 158 e mantém o Bloco 14 em recorte pequeno e auditável.
- A Pinker passa a oferecer `ler_linha_csv_bombom(linha, sep) -> lista<bombom>` e `emitir_linha_csv_bombom(itens, sep) -> verso` como primeiro núcleo tabular do projeto.
- O runtime exige separador explícito de 1 caractere e falha com erro claro para quoting, multiline e campos fora do recorte de `bombom`.
- `formatar_verso`, `falar(...)`, `verso`, coleções e o restante do pipeline continuam funcionais; regressão zero confirmada.
- O recorte permanece conservador: sem CSV RFC amplo, sem quoting complexo, sem multiline, sem cabeçalhos ricos, sem JSON, sem datas/tempo e sem serialização ampla.

## 3. Próximo passo correto
- Prosseguir no Bloco 14 apenas com recortes tão pequenos quanto este; o próximo passo natural passa a ser 14.3 (JSON básico), sem inflar o CSV recém-aberto nem reabrir o Bloco 13 por inércia.

## 4. Restrições explícitas
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
