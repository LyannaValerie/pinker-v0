# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 157 — formatação e dados estruturados: formatação simples de saída com placeholders mínimos (camada 1 conservadora)**.
- **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase funcional ativa passa a ser 157 e abre o Bloco 14 no menor recorte útil e auditável.
- A Pinker passa a oferecer `formatar_verso(modelo, a[, b]) -> verso`, com placeholders sequenciais `{}` e substituição controlada de `bombom`/`verso`.
- O runtime valida o modelo de forma explícita: braces fora de `{}` e contagem errada de placeholders/argumentos falham com erro claro.
- `falar(...)`, `verso`, coleções e o restante do pipeline continuam funcionais; regressão zero confirmada.
- O recorte permanece conservador: sem JSON, sem CSV, sem datas/tempo, sem placeholders nomeados, sem escape rico e sem engine de templates.

## 3. Próximo passo correto
- Prosseguir no Bloco 14 apenas com recortes tão pequenos quanto este; o próximo passo natural passa a ser 14.2 (CSV mínimo), sem inflar a formatação recém-aberta nem reabrir o Bloco 13 por inércia.

## 4. Restrições explícitas
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
