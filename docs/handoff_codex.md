# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 159 — formatação e dados estruturados: JSON básico plano e auditável (camada 1 conservadora)**.
- **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase funcional ativa passa a ser 159 e mantém o Bloco 14 em recorte pequeno e auditável.
- A Pinker passa a oferecer `ler_json_plano_bombom(json) -> mapa<verso,bombom>` e `emitir_json_plano_bombom(mapa) -> verso` como primeiro núcleo mínimo de objeto JSON do projeto.
- O runtime aceita apenas objeto JSON plano para `mapa<verso,bombom>`, com chaves sem escape rico, valores `bombom` e emissão determinística por ordenação de chave.
- `formatar_verso`, `falar(...)`, `verso`, coleções e o restante do pipeline continuam funcionais; regressão zero confirmada.
- O recorte permanece conservador: sem arrays, sem nesting, sem pretty print, sem escapes ricos, sem `true`/`false`/`null`, sem JSON geral, sem datas/tempo e sem serialização ampla.

## 3. Próximo passo correto
- Prosseguir no Bloco 14 apenas com recortes tão pequenos quanto este; o próximo passo natural passa a ser 14.4 (datas e tempo básicos), sem inflar o JSON recém-aberto nem reabrir o Bloco 13 por inércia.

## 4. Restrições explícitas
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
