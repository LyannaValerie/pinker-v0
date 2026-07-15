# Roadmap macro da Pinker (trilha oficial ativa)

- **Classe:** Engine
- **Papel:** referência
- **Status:** ativo

`docs/roadmap.md` é o topo executivo da ordem ativa oficial da Pinker v0.

## Papel deste arquivo

- preservar a trilha única oficialmente ativa;
- deixar inequívoco qual bloco está em curso;
- apontar para o índice e para os shards estruturais do roadmap;
- evitar que o roadmap volte a funcionar como crônica factual longa.

## Ordem ativa oficial

- A Pinker segue uma trilha única de execução.
- O bloco oficialmente ativo é o **Bloco 20 — expansão funcional rumo a SO e self-hosting (trilha por faixas)**, aberto na **Fase 207**.
- O bloco mais recentemente encerrado é o **Bloco 18 — core nobre e bibliotecas temáticas**, encerrado por suficiência conservadora na **Fase 207**.
- O **Bloco 19 — superfície Pinker** permanece candidato futuro, não ativo; a ativação do Bloco 20 antes dele é decisão estratégica explícita, subordinada aos dois propósitos de longo prazo do projeto.
- A frente pausada oficial permanece o **editor/TUI da Pinker**, aberta na **Fase 136** e não abandonada.

## Bloco ativo atual

**Bloco 20 — expansão funcional rumo a SO e self-hosting (trilha por faixas)**

**Tese estratégica**
Expandir a linguagem na direção dos dois propósitos de longo prazo: gerar um sistema operacional usando apenas Pinker e tornar a Pinker capaz de escrever o próprio código (self-hosting).

**Estado atual**
O bloco executa em **dois eixos** (Doc-41): **Eixo A — linguagem** (11 faixas, 52 itens inventariados frente a C, C#, C++, Python, TypeScript e Shell) e **Eixo B — paridade real do backend nativo** (fases B1–B11, Fases 212–222). Do Eixo A, os itens 1 (enums), 2 (pattern matching) e 3 (generics) da Faixa 1 foram entregues nas Fases 208–211; a Faixa 2 nasceu concluída pelo fechamento do Bloco 18. O **Eixo B está ENCERRADO** (Fase 222, paridade verificada em 155 exemplos); a trilha ativa volta ao Eixo A nos itens 5 (error handling), 6 (closures) e 4 (traits) da Faixa 1, agora com lowering nativo obrigatório em cada fase.

**Escada macro**
- Eixo A, Faixa 1 — funcionalidades de alta dificuldade (itens 1–3 entregues; 5, 6 e 4 após o Eixo B);
- **Eixo B — paridade real do backend nativo** (runtime próprio + lowering completo da superfície atual; **B1–B11 concluídos, Fases 212–222; eixo encerrado**);
- Eixo A, Faixa 3 — ponteiros de função, alocador de memória, inline assembly real;
- Eixo A, Faixas 4–6 — sistema de tipos, funções e controle de fluxo;
- Eixo A, Faixas 7–9 — baixo nível, metaprogramação, módulos e build;
- Eixo A, Faixas 10–11 — concorrência, SO, I/O e rede.

**Detalhe estrutural**
- `docs/roadmap/blocos/bloco_20.md`

## Relação com os demais documentos

- `docs/roadmap.md` define a ordem ativa.
- `docs/roadmap/indice.md` organiza a navegação curta por blocos.
- `docs/roadmap/blocos/bloco_XX.md` guardam o detalhe estrutural de cada bloco.
- `docs/history.md` e `docs/history/` preservam a crônica factual detalhada.
- `docs/future.md` continua sendo inventário técnico e não dita a ordem ativa.

## Navegação

- Hub do roadmap: `docs/roadmap/indice.md`
- Bloco ativo atual: `docs/roadmap/blocos/bloco_20.md`
- Bloco recém-encerrado: `docs/roadmap/blocos/bloco_18.md`
- Candidato futuro não ativo: `docs/roadmap/blocos/bloco_19.md`
