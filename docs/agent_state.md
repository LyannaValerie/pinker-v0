# Estado operacional da Pinker v0 (versão slim)

- **Classe:** Engine
- **Papel:** estado
- **Status:** operacional

## 1. Metadados do projeto
- Projeto: **Pinker v0**.
- Natureza: frontend/pipeline textual em Rust, com runtime interpretado em `--run`.
- Fonte de verdade: código local mergeado + documentação canônica do repositório.

## 2. Estado corrente
- Fase mais recente: **180 — core nobre e bibliotecas temáticas: inventário canônico de intrínsecas (abertura do Bloco 18)**.
- Bloco oficialmente ativo: **18 — core nobre e bibliotecas temáticas (aberto na Fase 180)**.
- Bloco documental mais recentemente encerrado: **17 — forma visual e superfície documental (encerrado por suficiência conservadora na Fase 176)**.
- Bloco funcional mais recentemente encerrado: **16 — ferramenta cotidiana madura e linguagem-cola (encerrado por suficiência conservadora na Fase 179)**.
- Frente pausada (oficial e não abandonada): **editor/TUI oficial da Pinker (aberto na Fase 136)**.
- Ajuste extraordinário corrente: promoção canônica de `tem_chave`, `pedir_argumento` e `buscar_contexto`, com legado temporário para `tem_argumento_nomeado`, `argumento_nomeado_ou` e `argumento_nomeado_ou_ambiente_ou`.
- Leitura canônica do estado: a Fase 180 abriu o **Bloco 18** com inventário canônico de 78 intrínsecas públicas, critérios explícitos de classificação e taxonomia inicial; nenhuma família pública foi operacionalizada; nenhuma mudança funcional foi feita.
- Escada interna do Bloco 18: **18.1 concluído (inventário e taxonomia canônica, Fase 180)**; 18.2–18.11 pendentes.
- Próximo passo funcional do Bloco 18: **18.2 — definição das famílias temáticas oficiais** (declarar famílias públicas, validação lexical, tratamento provisório de `colecao` e `formato`).

## 3. Arquitetura documental ativa
- `roadmap.md` = ordem ativa.
- `history.md` = crônica única.
- `agent_state.md` = estado corrente enxuto.
- `handoff_codex.md` = bilhete operacional curto.
- `atlas.md` = navegação mestre.
- `ponte_engine_rosa.md` = mediação estável Engine ↔ Rosa.
- `inventario_intrinsecas.md` = inventário canônico de intrínsecas (Bloco 18).
- `phases.md` = compatibilidade legada.

## 4. Restrições do projeto
- Não abrir fase funcional fora da ordem ativa do roadmap.
- Não transformar `future.md` em roadmap.
- Não transformar `parallel.md` em backlog técnico.
- Não declarar funcionalidade como pronta sem validação objetiva.
- Não operacionalizar famílias públicas antes da decisão lexical de 18.2.

## 5. Padrão operacional de binários
- Binário principal: `pink`.
- Binário MCP histórico (`pinker_mcp`) foi removido por segurança e não faz parte do estado operacional atual.
- Padrão recomendado: `cargo run --bin pink -- ...`.
