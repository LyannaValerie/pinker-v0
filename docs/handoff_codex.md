# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 156 — coleções e estruturas de dados básicas: aleatoriedade básica com semente explícita (camada 1 conservadora)**.
- **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase funcional ativa passa a ser 156 e encerra o Bloco 13 por suficiência conservadora.
- A Pinker passa a oferecer um núcleo mínimo de pseudoaleatoriedade reproduzível com semente explícita via `aleatorio_criar(semente)` e `aleatorio_proximo(gerador)`.
- O runtime mantém estado pequeno de geradores por handle, sem depender de tempo do sistema e com mesma semente -> mesma sequência.
- `lista<bombom>`, `mapa<verso,bombom>` e a iteração mínima do bloco continuam funcionais; regressão zero confirmada.
- O recorte permanece conservador: sem floats, sem distribuições ricas, sem shuffle, sem escolha aleatória sobre coleção e sem API criptográfica.

## 3. Próximo passo correto
- Iniciar o Bloco 14 no menor recorte útil e auditável (14.1 — formatação simples de saída), sem reabrir o Bloco 13 por inércia nem inflar a linguagem para biblioteca de dados ampla.

## 4. Restrições explícitas
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
