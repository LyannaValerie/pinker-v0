# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Fase 141 — ergonomia prática de script: argumentos nomeados mínimos (camada 1 conservadora)**.
- Quinta fase funcional do Bloco 11; abre um primeiro degrau mínimo e auditável de parsing nomeado em `--run`, em continuidade direta do eixo 11.5.

## 2. Resultado operacional da rodada
- Dois novos intrínsecos adicionados ao runtime `--run`:
  - `tem_argumento_nomeado(chave: verso) -> logica`
  - `argumento_nomeado_ou(chave: verso, padrao: verso) -> verso`
- Recorte suportado: apenas `--chave valor` e `--chave=valor`, com busca literal no vetor de argv já repassado após `--`.
- `tem_argumento_nomeado` sinaliza presença apenas nas formas suportadas.
- `argumento_nomeado_ou` retorna o valor associado quando presente, usa fallback quando ausente e falha com erro claro se `--chave` aparecer sem valor na forma separada.
- Chave vazia é rejeitada em runtime com erro claro para evitar ambiguidade operacional.
- Resultado representado em tipos já existentes (`logica` e `verso`): nenhum novo tipo estrutural introduzido.
- Testes semânticos + de runtime + de CLI adicionados com regressão zero.
- Exemplo canônico criado: `examples/fase141_argumentos_nomeados_minimos_valido.pink`.

## 3. Continuidade preservada
- Fase funcional atual: **141**.
- Fase funcional anterior: **140**.
- Rodada documental mais recente: **Doc-25**.

## 4. Próximo passo correto
- Seguir em 11.5 com degraus pequenos de ergonomia prática de script após os argumentos nomeados mínimos.
- Não continuar o editor/TUI agora; a frente está pausada por decisão estratégica e não abandonada.
- Não reabrir o Bloco 10 por impulso; qualquer retorno a 10.1–10.6 segue excepcional, pequeno e bem justificado.

## 5. Restrições explícitas
- Sem backend nativo pleno por declaração documental.
- Sem ABI ampla/plena, sem sistema geral de strings/texto, sem sistema geral de layout/campos e sem casts gerais entre todos os tipos.
- Sem continuidade funcional do editor/TUI nesta rodada documental.
- Sem REPL, linguagem-cola, execução de processos externos ou integração rica de stdio como foco imediato do bloco.
