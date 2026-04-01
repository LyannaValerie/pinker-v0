# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 168 — argv explícito mínimo para processo externo (camada 1 conservadora)**.
- Fase funcional ativa corrente: **168** (15.1, 15.2, 15.3, 15.4, 15.5 e 16.1 já concluídos no recorte mínimo; 16.2 aberta em camada 1 conservadora).
- Rodada extraordinária corrente: **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase 168 abre o primeiro degrau funcional real de 16.2 com `argv` explícito mínimo em `executar_processo(comando, argv1)`.
- O runtime continua sem shell implícito: o comando segue separado do argumento textual explícito e o recorte aceita exatamente um `argv1`.
- O desenho reaproveita a infraestrutura mínima já aberta no Bloco 15 e preserva honestidade factual: não há parser amplo de shell, quoting/escaping rico, listas gerais de argumentos, PTY, sessão interativa nem expansão automática para `capturar_*`, `executar_com_entrada` ou `pipeline_minimo`.
- Testes cobrem caso positivo básico, fluxo composto, erro claro fora do recorte mínimo e preservação dos caminhos anteriores de subprocesso.

## 3. Próximo passo correto
- Seguir em **16.2 — linguagem-cola** por degraus pequenos e auditáveis, ampliando subprocessos apenas quando houver valor real e desenho explícito, sem virar shell adulta, argv amplo por inércia, parser de comando inteiro, quoting rico, pipeline longo ou REPL adulto.

## 4. Restrições explícitas
- Sem reabrir Bloco 14 por inércia; CSV/JSON/tempo/formatação amplos pertencem ao futuro quando justificados, não à continuação automática.
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem reabrir subprocessos por inércia: `stderr`, `stdin` e `pipe` foram entregues em degraus pequenos; isso não equivale a integração completa de processos nem autoriza shell amplo, pipeline longo, sessão interativa, PTY ou job control por inércia.
- Sem inflar a Fase 167: o REPL atual não autoriza por inércia multiline amplo, histórico sofisticado, autocomplete, comandos administrativos amplos, inspeção rica ou persistência de estado entre linhas.
- Sem inflar a Fase 168: o novo `argv1` explícito não autoriza por inércia listas gerais de argumentos, quoting/escaping rico, shell mode, `stdin` interativo, PTY, job control ou expansão automática de toda a família de subprocessos.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
