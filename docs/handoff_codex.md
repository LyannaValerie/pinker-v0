# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 138 — manipulação textual útil: `replace` (camada 1 conservadora)**.
- Segunda fase funcional do Bloco 11; abre o item 11.2 (substituição textual útil) com o menor degrau prático auditável.

## 2. Resultado operacional da rodada
- Um novo intrínseco textual adicionado ao runtime `--run`:
  - `substituir_verso(texto, de, para) -> verso`: substitui globalmente todas as ocorrências literais de `de` por `para` em `texto`.
- Padrão `de` vazio é rejeitado em runtime; `para` pode ser vazio (remove ocorrências).
- Substituição literal pura, sem regex, sem flags, sem múltiplos modos, sem abertura de `join` nesta fase.
- Resultado representado em tipo já existente (`verso`): nenhum novo tipo estrutural introduzido.
- 7 novos testes adicionados à suíte de intérprete; regressão zero (todas as fases anteriores continuam verdes).
- Exemplo canônico criado: `examples/fase138_replace_camada1_valido.pink`.

## 3. Continuidade preservada
- Fase funcional atual: **138**.
- Fase funcional anterior: **137**.
- Rodada documental mais recente: **Doc-25**.

## 4. Próximo passo correto
- Continuar o eixo textual com `join` (camada 1 conservadora), ou avançar para 11.3 (utilitários práticos de arquivo/caminho/ambiente) conforme prioridade real.
- Não continuar o editor/TUI agora; a frente está pausada por decisão estratégica e não abandonada.
- Não reabrir o Bloco 10 por impulso; qualquer retorno a 10.1–10.6 segue excepcional, pequeno e bem justificado.

## 5. Restrições explícitas
- Sem backend nativo pleno por declaração documental.
- Sem ABI ampla/plena, sem sistema geral de strings/texto, sem sistema geral de layout/campos e sem casts gerais entre todos os tipos.
- Sem continuidade funcional do editor/TUI nesta rodada documental.
- Sem REPL, linguagem-cola, execução de processos externos ou integração rica de stdio como foco imediato do bloco.
