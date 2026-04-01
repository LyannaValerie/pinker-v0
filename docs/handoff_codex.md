# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Doc-31 — fechamento canônico do Bloco 15 por suficiência conservadora**.
- Fase funcional ativa corrente: **166** (15.1, 15.2, 15.3, 15.4 e 15.5 já concluídos no recorte mínimo).
- Rodada extraordinária corrente: **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase 161 abriu o Bloco 15 com o menor recorte útil de integração sistêmica: `executar_processo(comando) -> bombom`.
- A Fase 162 preservou esse recorte e corrigiu a portabilidade prática de testes/exemplos.
- A Fase 163 abriu `capturar_stdout(comando) -> verso` no mesmo desenho conservador: um único comando/caminho em `verso`, sem shell implícito, com UTF-8 estrito e retorno apenas do stdout textual.
- A Fase 164 abriu `capturar_stderr(comando) -> verso` como espelho conservador de 15.2: um único comando/caminho em `verso`, sem shell implícito, com UTF-8 estrito e retorno apenas do stderr textual.
- A Fase 165 abriu `executar_com_entrada(comando, entrada) -> bombom` como recorte mínimo de stdin textual: um único comando/caminho em `verso`, uma única escrita textual em stdin, sem shell implícito, sem sessão interativa e com retorno apenas do código de saída.
- A Fase 166 abriu `pipeline_minimo(produtor, consumidor) -> bombom` como primeiro recorte de composição direta entre processos: dois comandos/caminhos explícitos, stdout do produtor ligado ao stdin do consumidor, sem shell implícito, sem cadeia longa e com retorno apenas do código de saída do consumidor.
- A Doc-31 fecha canonicamente o Bloco 15 por suficiência conservadora: 15.1–15.5 foram entregues no recorte mínimo, o primeiro conjunto útil de linguagem-cola sistêmica ficou completo e subprocessos amplos continuam fora do que foi entregue.

## 3. Próximo passo correto
- Abrir o **Bloco 16 — ferramenta cotidiana madura e linguagem-cola** como próxima trilha formal, sem reabrir subprocessos por inércia e sem inflar a Fase 166 retroativamente para shell adulto, pipeline amplo ou integração completa de subprocessos.

## 4. Restrições explícitas
- Sem reabrir Bloco 14 por inércia; CSV/JSON/tempo/formatação amplos pertencem ao futuro quando justificados, não à continuação automática.
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem reabrir subprocessos por inércia: `stderr`, `stdin` e `pipe` foram entregues em degraus pequenos; isso não equivale a integração completa de processos nem autoriza shell amplo, pipeline longo, sessão interativa, PTY ou job control por inércia.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
