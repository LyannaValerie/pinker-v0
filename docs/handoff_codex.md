# Handoff Codex (operacional curto)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Rodada atual
- **Doc-36 — refatoração estrutural AX-friendly do roadmap**.
- Leitura operacional canônica: a rodada é estritamente documental e reorganiza o roadmap canônico em topo executivo, índice e shards por bloco, sem abrir nova funcionalidade de linguagem.

## 2. Resultado operacional da rodada
- `docs/roadmap.md` virou topo executivo curto da ordem ativa.
- `docs/roadmap/indice.md` passou a ser o hub de navegação por blocos.
- `docs/roadmap/blocos/` passou a concentrar o detalhe estrutural de cada bloco em shards uniformes.
- A duplicação entre roadmap e histórico foi reduzida: o roadmap deixou de carregar encerramentos narrativos longos e tese/executivo de todos os blocos no mesmo arquivo.
- Não houve mudança funcional em parser, semântica, runtime, `src/`, `tests/` ou exemplos.

## 3. Próximo passo correto
- Próximo passo documental provável: continuar o Bloco 18 pela família exemplar `tempo`, usando a nova arquitetura do roadmap sem voltar a inflar `docs/roadmap.md`.
- O **Bloco 18** segue como bloco oficialmente ativo em camada documental/arquitetural.

## 4. Restrições explícitas
- Sem tratar a nova arquitetura do roadmap como reabertura de fase funcional.
- Sem voltar a inflar `docs/roadmap.md` ou `docs/roadmap/indice.md` como se fossem crônicas completas.
- Sem implementar reformas sintáticas ou reorganização de engine por inércia documental.
- Sem inflar `future.md` como se fosse roadmap ou `parallel.md` como se fosse backlog técnico executável.
