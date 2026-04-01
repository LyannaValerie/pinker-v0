# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 167 — REPL mínimo auditável (camada 1 conservadora)**.
- Fase funcional ativa corrente: **167** (15.1, 15.2, 15.3, 15.4, 15.5 e 16.1 já concluídos no recorte mínimo).
- Rodada extraordinária corrente: **FE-1 — refino lexical extraordinário: aquecer a periferia utilitária do runtime (camada 1 conservadora)**.

## 2. Resultado operacional da rodada
- A Fase 167 abre o Bloco 16 com o menor recorte útil de interatividade: `pink repl`.
- O REPL reaproveita o pipeline real da Pinker até o runtime existente; não há arquitetura paralela de execução.
- Recorte entregue de forma explícita: cada linha vira o corpo temporário de `principal`, sem estado persistente entre linhas, sem multiline amplo e com saída por `:quit`/`:sair`.
- O desenho evita replay implícito de efeitos colaterais entre submissões e preserva honestidade factual: não é shell adulta, não é editor rico e não é ambiente interativo completo.
- Testes cobrem uso mínimo, fluxo composto em uma linha, recuperação após entrada inválida e comando de saída, sem regressão dos modos CLI já existentes.

## 3. Próximo passo correto
- Seguir para **16.2 — linguagem-cola** em degrau pequeno e auditável, sem inflar a Fase 167 para REPL adulto, sessão persistente rica, multiline amplo, autocomplete, shell ampla ou editor integrado.

## 4. Restrições explícitas
- Sem reabrir Bloco 14 por inércia; CSV/JSON/tempo/formatação amplos pertencem ao futuro quando justificados, não à continuação automática.
- Sem reabrir Bloco 11 por inércia documental; qualquer retorno ao tema deve ser excepcional e bem justificado.
- Sem reabrir Bloco 12 por inércia; futuras ampliações de módulos devem ser pequenas, explicitamente justificadas e fora de continuação automática.
- Sem reabrir Bloco 10 por inércia documental.
- Sem reabrir subprocessos por inércia: `stderr`, `stdin` e `pipe` foram entregues em degraus pequenos; isso não equivale a integração completa de processos nem autoriza shell amplo, pipeline longo, sessão interativa, PTY ou job control por inércia.
- Sem inflar a Fase 167: o REPL atual não autoriza por inércia multiline amplo, histórico sofisticado, autocomplete, comandos administrativos amplos, inspeção rica ou persistência de estado entre linhas.
- Sem transformar `future.md` em roadmap ou `parallel.md` em backlog técnico.
