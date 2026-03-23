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
- **Paralela-N**: rodada paralela de implementação (sem conflito com trilha ativa, sem ser hotfix nem fase funcional).
- Histórico detalhado fica em `docs/history.md`.

## 4. Pipeline congelada
- Fluxo base: semântica -> IR -> validação IR -> CFG IR -> validação CFG -> selected -> validação selected -> Machine -> validação Machine.
- Saídas: `--pseudo-asm`, `--asm-s`, `--run` (cada modo respeitando seu caminho interno atual).
- Estado: sem backend nativo completo e sem redesign estrutural aberto nesta rodada.

## 5. Estado corrente
- Fase funcional atual: **95 — ambiente mínimo de processo em `--run` (fallback de env + diretório atual)**.
- Rodada documental mais recente: **Doc-16 — pacote paralelo de apoio (auditoria + corpus + mapeamento de codegen textual)**.
- Fase funcional anterior: **94 — refinamento mínimo de argv em `--run` (fallback posicional simples)**.
- Bloco concluído: **Bloco 6 — Memória operacional** (Fases 64–72 entregues).
- Bloco ativo: **Bloco 8 — I/O e ecossistema útil**.
- Bloco 7: **suficientemente consolidado para transição**, sem declaração de completude absoluta.
- Próximo passo funcional sugerido: **seguir refinamentos mínimos de tooling em `--run` no Bloco 8, preservando recorte pequeno e auditável sem parser amplo de CLI**.
- Eixo `--asm-s`/backend externo permanece em subset linear auditável (Fases 73–84), mapeado nesta rodada sem abertura de lowering novo.
- Rodada funcional corrente: **nenhuma aberta** (última fase funcional concluída é a Fase 95).
- Última rodada paralela concluída: **Paralela-1 — negação bitwise dual (`~` + `nope`) + MCP mínimo**.
- Último hotfix aplicado: **HF-2 — varredura de corretude do Bloco 6 (Fases 64–70)**.

## 6. Ecossistema documental
- `docs/roadmap.md`: trilha ativa oficial.
- `manual.md`: manual de uso da linguagem Pinker no estado atual.
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
