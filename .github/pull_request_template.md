<!--
Template de PR da Trama Pinker (especificação §13).

Durante o regime temporário de maturação adulta + modularização, a documentação
canônica permanece congelada. O corpo narrativo da PR deve ser curto: o quê,
onde, como e por quê. Conhecimento operacional reutilizável vai para o Book, sob a
autoridade própria dele, conforme AGENTS.md.

Nenhum bloco estruturado é exigido de um PR. O acervo `.pinker/changes/` é
histórico e finito; ele não recebe manifesto novo (Issue #698).

O CI valida em modo somente leitura; ele não sincroniza nem cria commits.
-->

## Registro mínimo

- **O que:** <!-- mudança realizada -->
- **Onde:** <!-- arquivos/módulos/superfícies afetados -->
- **Como:** <!-- abordagem técnica, em poucas linhas -->
- **Por quê:** <!-- causa/necessidade que justifica a mudança -->

<!-- Seja breve. Não transforme a PR em documentação paralela. -->

## Validação

<!-- Marque somente comandos realmente executados e registre limitações reais. -->

- [ ] `make ci`
- [ ] `git diff --check`

## Conhecimento operacional

<!--
Opcional. Não é diário, e nenhuma PR é obrigada a produzir retenção.

Book                 = destino de nova retenção operacional reutilizável,
                       somente sob a autoridade própria aplicável do Book
<TASK_ROOT>/memory   = memória factual da Task em JSON/JSONL, dentro do root
                       observado pelo `forja-agentes`
<TASK_ROOT>/state    = checkpoint e estado de retomada
<TASK_ROOT>/artifacts = evidência e artefatos da Task

Se houve retenção aplicável, aponte a referência do que foi retido — o id do caso,
ou o receipt da Task Book quando houver uma. Caso contrário, `nenhuma`. Não copie o
conteúdo retido para a PR; as regras completas estão na seção "Conhecimento
operacional" do AGENTS.md e na governança do Book.
-->

- Retenção: `nenhuma`
