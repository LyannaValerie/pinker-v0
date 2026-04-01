# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 161 — processos e integração sistêmica: execução mínima de processo externo com código de saída (camada 1 conservadora)**.
- Fase funcional ativa corrente: **161** (primeiro marco funcional do Bloco 15).
- Rodada extraordinária corrente: **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase 161 abriu o Bloco 15 com o menor recorte útil de integração sistêmica: `executar_processo(comando) -> bombom`.
- O recorte entregue é pequeno e auditável: um único comando/caminho em `verso`, execução síncrona mínima, sem shell implícito e retorno apenas do código de saída.
- O resultado cobre automação básica por status sem inflar a superfície para shell adulto, captura de saída, redirecionamento, pipes, ambiente/cwd customizados ou controle avançado de processo.

## 3. Próximo passo correto
- Abrir 15.2 — captura mínima de stdout, sem reabrir o Bloco 14 nem inflar 15.1 retroativamente.

## 4. Restrições explícitas
- Sem reabrir Bloco 14 por inércia; CSV/JSON/tempo/formatação amplos pertencem ao futuro quando justificados, não à continuação automática.
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
