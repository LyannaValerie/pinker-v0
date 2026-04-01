# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Doc-30 — refino da escada interna do Bloco 15 para subdegraus pequenos e auditáveis**.
- Fase funcional ativa corrente: **163** (15.1 e 15.2 já concluídos no recorte mínimo; nenhuma fase funcional nova foi aberta nesta rodada).
- Rodada extraordinária corrente: **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase 161 abriu o Bloco 15 com o menor recorte útil de integração sistêmica: `executar_processo(comando) -> bombom`.
- A Fase 162 preservou esse recorte e corrigiu a portabilidade prática de testes/exemplos.
- A Fase 163 abriu `capturar_stdout(comando) -> verso` no mesmo desenho conservador: um único comando/caminho em `verso`, sem shell implícito, com UTF-8 estrito e retorno apenas do stdout textual.
- A Doc-30 não abriu nova fase funcional: apenas refinou o antigo 15.3 em subdegraus menores (`stderr`, `stdin` e `pipe` mínimos), preservando honestidade factual sobre o que o bloco ainda não entregou.

## 3. Próximo passo correto
- Abrir **15.3 — captura mínima de stderr como `verso`**, sem inflar 15.2 retroativamente e sem tratar integração completa de subprocessos como se já estivesse entregue.

## 4. Restrições explícitas
- Sem reabrir Bloco 14 por inércia; CSV/JSON/tempo/formatação amplos pertencem ao futuro quando justificados, não à continuação automática.
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem inflar subprocessos por inércia: `stderr`, `stdin` e `pipe` seguem separados em degraus pequenos; 15.1 e 15.2 não equivalem a integração completa de processos.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
