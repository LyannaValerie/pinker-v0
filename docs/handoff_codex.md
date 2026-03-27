# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 137 — manipulação textual útil: `split` (camada 1 conservadora)**.
- Primeira fase funcional do Bloco 11; abre o item 11.1 (texto útil) com o menor degrau prático auditável.

## 2. Resultado operacional da rodada
- Dois novos intrínsecos textuais adicionados ao runtime `--run`:
  - `dividir_verso_em(texto, sep, indice) -> verso`: retorna o N-ésimo pedaço (base 0) da divisão de `texto` por `sep`.
  - `dividir_verso_contar(texto, sep) -> bombom`: retorna o número de pedaços.
- Separador simples em `verso`, sem regex, sem coleção geral, sem abertura de `replace`/`join`.
- Resultado representado em tipos já existentes (`verso` e `bombom`): nenhum novo tipo estrutural introduzido.
- 11 novos testes adicionados à suíte de intérprete; regressão zero (todas as fases anteriores continuam verdes).
- Exemplo canônico criado: `examples/fase137_split_camada1_valido.pink`.

## 3. Continuidade preservada
- Fase funcional atual: **137**.
- Fase funcional anterior: **136**.
- Rodada documental mais recente: **Doc-25**.

## 4. Próximo passo correto
- Continuar o item 11.1 com `replace` ou `join` (camada 1 conservadora), ou avançar para 11.2 (scripts e CLI) conforme prioridade real.
- Não continuar o editor/TUI agora; a frente está pausada por decisão estratégica e não abandonada.
- Não reabrir o Bloco 10 por impulso; qualquer retorno a 10.1–10.6 segue excepcional, pequeno e bem justificado.

## 5. Restrições explícitas
- Sem backend nativo pleno por declaração documental.
- Sem ABI ampla/plena, sem sistema geral de strings/texto, sem sistema geral de layout/campos e sem casts gerais entre todos os tipos.
- Sem continuidade funcional do editor/TUI nesta rodada documental.
- Sem REPL, linguagem-cola, execução de processos externos ou integração rica de stdio como foco imediato do bloco.
