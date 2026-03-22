# Estado operacional da Pinker v0 (versão slim)

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código mergeado + documentação ativa do repositório local.

## 2. Diretrizes consolidadas de execução
- Manter fases pequenas, auditáveis e coerentes com `docs/roadmap.md`.
- Evitar refactor amplo fora do escopo da rodada.
- Preservar continuidade histórica e não reabrir fase concluída.
- Em conflito, código mergeado prevalece sobre documentação.

## 3. Convenção de fases/rodadas
- **Fase N**: entrega funcional real.
- **HF-N**: hotfix extraordinário sem abrir nova fase funcional.
- **Doc-N**: rodada exclusivamente documental.
- Histórico detalhado fica em `docs/history.md`.

## 4. Pipeline congelada
- Fluxo base: semântica -> IR -> validação IR -> CFG IR -> validação CFG -> selected -> validação selected -> Machine -> validação Machine.
- Saídas: `--pseudo-asm`, `--asm-s`, `--run` (cada modo respeitando seu caminho interno atual).
- Estado: sem backend nativo completo e sem redesign estrutural aberto nesta rodada.

## 5. Estado corrente
- Fase funcional atual: **74 — convenção de chamada concreta mínima**.
- Fase funcional anterior: **73 — subset real montável ampliado**.
- Bloco concluído: **Bloco 6 — Memória operacional** (Fases 64–72 entregues).
- Próximo bloco ativo: **Bloco 7 — Backend nativo real**.
- Bloco futuro já definido (não ativo): **Bloco 8 — I/O e ecossistema útil**.
- Item normal sugerido após a Fase 74: **frame/registradores mínimos reais** (Bloco 7).
- Rodada documental corrente: **Doc-11 — abertura documental dos Blocos 7 e 8**.
- Último hotfix aplicado: **HF-2 — varredura de corretude do Bloco 6 (Fases 64–70)**.

## 6. Ecossistema documental
- `docs/roadmap.md`: trilha ativa oficial.
- `docs/future.md`: inventário técnico amplo de longo prazo.
- `docs/parallel.md`: visão/fantasia orientadora, sem ditar ordem ativa.
- `docs/history.md`: crônica histórica única (fases/hotfixes/documentação).
- `docs/handoff_codex.md`: handoff operacional curto da rodada.
- `docs/doc_rules.md`: convenções obrigatórias de documentação.

## 7. Infraestrutura mínima ativa
- Toolchain com MSRV fixada (`rust-toolchain.toml`).
- CI com build/check/fmt/clippy/test/doc.
- Validação local padrão com `cargo build` e `cargo test` (e variantes `--locked` quando requerido).

## 8. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.

## 9. Itens adiados
- Escrita de campo em `ninho`, acesso por valor `p.campo` e extensão para campos não escalares.
- Escrita por índice em arrays e suporte operacional além do subset `(*ptr)[i]` com `[bombom; N]`.
- Cast operacional além do subset da Fase 71 (`bombom <-> seta<bombom>` e inteiro->inteiro), incluindo `seta<T> -> bombom` genérico e casts entre compostos.
- Efeito operacional robusto/completo de `fragil` (MMIO, fences, ordenação de memória e backend nativo).
- Backend nativo completo e runtime bare-metal robusto.

## 10. Instrução para novo agente
1. Ler: `README.md`, `docs/roadmap.md`, `docs/agent_state.md`, `docs/handoff_codex.md`, `docs/history.md`, `docs/doc_rules.md`.
2. Executar validações exigidas da rodada antes de encerrar.
3. Atualizar ao final: `docs/history.md`, `docs/agent_state.md`, `docs/handoff_codex.md` quando houver mudança documental/operacional.
