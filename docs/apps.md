# Aplicacoes Pinker Internas

- **Classe:** Engine
- **Papel:** regra
- **Status:** ativo

`apps/` e o espaco para programas escritos em Pinker que ajudam o desenvolvimento da propria linguagem.

## Papel

Um app interno nao e exemplo pequeno nem teste isolado. Ele precisa:

- resolver uma necessidade real do fluxo de desenvolvimento;
- ter comando de execucao documentado;
- ter contrato claro de entrada, saida e status;
- usar a superficie implementada da Pinker sem declarar recurso futuro como pronto;
- entrar no CI quando fizer parte do fluxo ativo.

## Diferenca entre apps, exemplos e testes

| Categoria | Papel |
|---|---|
| `examples/` | demonstrar recursos versionados e pequenos |
| `tests/` | validar comportamento automaticamente |
| `apps/` | operar sobre o proprio repositorio ou produzir artefatos uteis |

## Fluxo ativo

O primeiro app ativo e `apps/guardiao_pinker/principal.pink`.

Antes de iniciar a proxima etapa do Bloco 20, Eixo A, o fluxo recomendado passa a ser:

```bash
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .
make ci
```

O Guardiao Pinker nao substitui `make ci`; ele cobre contratos editoriais e operacionais que historicamente causaram desalinhamento entre README, handoff, roadmap e historico.

Na validacao de fase funcional, a fase esperada nao deve ficar fixa no codigo do app. O Guardiao deriva a fase atual de `docs/handoff_codex.md` e confere se o mesmo numero aparece na porta publica (`README.md`), no indice historico de fases, no shard historico ativo e no roadmap.

O app tambem possui modo consultivo para olhar um arquivo especifico sem rodar todas as regras:

```bash
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo . --docs --arquivo docs/handoff_codex.md --status fase --fase 239
./ci_env.sh cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo . --src --arquivo src/cfg_ir.rs --status busca --busca lower_short_circuit_value
```

No recorte atual, argumentos nomeados seguem o formato `--chave valor`; formas compactas como `--fase:239` ficam para uma expansao futura da ergonomia de argv.

## Evolucao

Novos apps podem entrar quando houver dor concreta no desenvolvimento. Cada app novo deve justificar:

- qual tarefa manual ele reduz;
- quais recursos da Pinker ele exercita;
- qual limite atual da linguagem ele torna visivel;
- qual teste garante que ele continua executavel.
