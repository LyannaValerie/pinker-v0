<!--
Template de PR da Trama Pinker (especificação §13).

Durante o regime temporário de maturação adulta + modularização, a documentação
canônica permanece congelada. O corpo narrativo da PR deve ser curto: o quê,
onde, como e por quê. Conhecimento operacional reutilizável vai para o Book, sob a
autoridade própria dele, conforme AGENTS.md.

O bloco ```pinker-change``` no final é voltado à AUTOMAÇÃO e deve permanecer
separado da narrativa. Para PRs posteriores ao marco #330, importe o bloco com:

    ./ci_env.sh cargo run --bin pink -- doc importar-pr <n> --corpo <arquivo>

Em Task da Forja, esse comando roda Cargo: conclua antes o "Bootstrap de Task" do
AGENTS.md, em particular exportar CARGO_TARGET_DIR. Sem isso o build cai em
<worktree>/target, fora do escopo que o finalizador sabe liberar.

O CI valida em modo somente leitura (`--check`); ele não sincroniza nem cria
commits.
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

## Bloco estruturado

<!--
Preencha o bloco abaixo apenas para mudanças posteriores ao marco #330.
Mantenha-o separado da narrativa acima. Campos e enums seguem
.pinker/schemas/change-v1.schema.json.

Propósito: fornecer à automação metadados verificáveis sem tentar interpretar o
resumo humano. Preencha `kind`, `title`, `status` e `area`; use `updates` somente
para famílias de projeção configuradas em `.pinker/doc.toml` e marque `true`
apenas quando a mudança declarar atualização daquela família. Não derive flags
de nomes de arquivos comuns, como `README.md`. Se a classificação não estiver
clara, peça orientação à manutenção antes de inventar uma fase, hotfix ou rodada
documental.

NÃO deixe comentários dentro do bloco ```pinker-change```: ele é lido pela
automação, não pelo YAML padrão. Substitua TODAS as sentinelas <preencher-...>
por valores reais antes de abrir ou atualizar o PR — sentinelas remanescentes
falham no CI com E-CHANGE-PLACEHOLDER.

Valores aceitos:
  kind:   phase | hotfix | documentation | parallel-phase
  status: completed | in-progress | planned
  area:   ids semânticos de território/domínio, ex.: development.trama,
          language.result (formato [a-z0-9]+([._-][a-z0-9]+)*)
-->

```pinker-change
schema: 1
kind: <preencher-kind>
title: <preencher-titulo>
status: <preencher-status>
area:
  - <preencher-area>
updates:
  state: false
  history: false
  roadmap: false
validation:
  required:
    - make ci
```
