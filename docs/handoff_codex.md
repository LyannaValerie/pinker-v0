# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 140 — manipulação textual útil: busca textual mínima (camada 1 conservadora)**.
- Quarta fase funcional do Bloco 11; abre o item 11.4 (busca textual útil) com o menor degrau prático auditável.

## 2. Resultado operacional da rodada
- Um novo intrínseco textual adicionado ao runtime `--run`:
  - `buscar_verso(texto, padrao) -> bombom`: retorna a posição da primeira ocorrência literal de `padrao` em `texto`.
- Ambos os argumentos são `verso`.
- Caso ausente retorna sentinela explícita `18446744073709551615` (`u64::MAX`), mantendo coerência com o estilo posicional já adotado.
- Padrão vazio é rejeitado com erro claro para evitar ambiguidade no recorte mínimo.
- Resultado representado em tipo já existente (`bombom`): nenhum novo tipo estrutural introduzido.
- Testes semânticos + de runtime + de CLI adicionados com regressão zero.
- Exemplo canônico criado: `examples/fase140_busca_textual_camada1_valido.pink`.

## 3. Continuidade preservada
- Fase funcional atual: **140**.
- Fase funcional anterior: **139**.
- Rodada documental mais recente: **Doc-25**.

## 4. Próximo passo correto
- Avançar para 11.5 (utilitários práticos de arquivo/caminho/ambiente e ergonomia de script) conforme prioridade real.
- Não continuar o editor/TUI agora; a frente está pausada por decisão estratégica e não abandonada.
- Não reabrir o Bloco 10 por impulso; qualquer retorno a 10.1–10.6 segue excepcional, pequeno e bem justificado.

## 5. Restrições explícitas
- Sem backend nativo pleno por declaração documental.
- Sem ABI ampla/plena, sem sistema geral de strings/texto, sem sistema geral de layout/campos e sem casts gerais entre todos os tipos.
- Sem continuidade funcional do editor/TUI nesta rodada documental.
- Sem REPL, linguagem-cola, execução de processos externos ou integração rica de stdio como foco imediato do bloco.
