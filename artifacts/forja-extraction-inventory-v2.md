# Forja extraction inventory v2

Source: `ev-550-artifacts/artifacts/forja-extraction-inventory.json`. Revalidation baseline: `3bde46b5fd7fe092f34a7d2ad1d344d5b8235339`.

Total: 49
Gen1 counts: {"FORJA_ORGANIZATIONAL":23,"MINIMAL_BRIDGE":2,"MIXED":12,"PINKER_PRODUCT":11,"UNCERTAIN":1}

| # | Path | Symbol/region | Gen1 class | Current status | Disposition |
|---:|---|---|---|---|---|
| 1 | `scripts/forja/verificar-paths.sh` | script inteiro (87 linhas, sem regiao nav) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 2 | `agentes/.gitignore` | '*' + excecoes .gitignore/README.md | MINIMAL_BRIDGE | UNCHANGED_SINCE_GEN1 | keep |
| 3 | `agentes/README.md` | 48 linhas de politica/documentacao do layout de Task | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | split — encolher a ~6 linhas de ponteiro (o que e o diretorio, quem e a autoridade, onde ela vive) |
| 4 | `AGENTS.md` | secoes 'Bootstrap de Task', 'Onde vive a Forja', 'Pressao de caminho e SUN_LEN', comandos de teste host-side | MIXED | UNCHANGED_SINCE_GEN1 | split — manter caminho canonico da Forja, entrypoint de invocacao, disciplina de lifecycle e ponteiro de autoridade; retirar manual de implementacao, arquitetura interna, politica host duplicada e a receita SUN_LEN |
| 5 | `Makefile` | alvo forja-paths-check e sua presenca em 'ci' | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 6 | `Makefile` | alvo cleanup-native | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 7 | `.github/pull_request_template.md` | 2 mencoes: 'Em Task da Forja... exportar CARGO_TARGET_DIR' e '<TASK_ROOT>/... observado pelo forja-agentes' | MIXED | UNCHANGED_SINCE_GEN1 | split — reduzir a um ponteiro; a instrucao de CARGO_TARGET_DIR e politica host duplicada |
| 8 | `.gitignore` | comentario '# Bytecode Python das ferramentas da Forja' | MINIMAL_BRIDGE | UNCHANGED_SINCE_GEN1 | keep |
| 9 | `scripts/pink-baseline` | script inteiro (790 linhas): build / manifest / ficha / publish | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 10 | `tests/f2_forja_integration_tests.rs` | regiao evidencia.tooling.f2.registros-derivados (linhas 18-2650), 49 #[test] | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 11 | `src/agent.rs` | development.agent.spec (src/agent.rs:17-761) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 12 | `src/agent.rs` | development.agent.paths (src/agent.rs:763-882) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 13 | `src/agent.rs` | development.agent.artifacts (src/agent.rs:884-1275) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 14 | `src/agent.rs` | development.agent.runner (src/agent.rs:1277-1530) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 15 | `src/agent.rs` | development.agent.git-diff (src/agent.rs:1532-1585) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 16 | `src/agent.rs` | development.agent.marker-only (src/agent.rs:1587-1782) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 17 | `src/agent.rs` | development.agent.projection (src/agent.rs:1784-1861) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 18 | `src/agent.rs` | development.agent.pr-body (src/agent.rs:1863-2292) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 19 | `src/agent.rs` | development.agent.contract-v1 (src/agent.rs:2294-2387) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 20 | `src/agent.rs` | development.agent.publication (src/agent.rs:2782-2977) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 21 | `src/agent.rs` | development.agent.remote-checks (src/agent.rs:2985-3173) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 22 | `src/agent.rs` | development.agent.resume (src/agent.rs:3206-3358) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 23 | `src/agent.rs` | development.agent.sensitivity (src/agent.rs:3360-3517) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 24 | `src/agent.rs` | development.agent.lifecycle (src/agent.rs:3519-3882) | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 25 | `src/agent.rs` | pub use pinker_sha256_contract::sha256_hex (linha 1248) | MIXED | UNCHANGED_SINCE_GEN1 | split — os consumidores devem passar a usar pinker_sha256_contract::sha256_hex diretamente |
| 26 | `src/main.rs` | enum AgentSub, agent_usage(), roteamento 'agente', 8 chamadas agent::*, '--agente-spec', 'agente' no help | MIXED | UNCHANGED_SINCE_GEN1 | split — retirar o subcomando; o resto de main.rs e produto |
| 27 | `src/project_state.rs` | DomainId::Agent, SourceKind::AgentSpec, collect_agent(), agent_publication_is_pending() | MIXED | UNCHANGED_SINCE_GEN1 | split — remover o dominio agent; repository/trama/documentation/projections/local_checks/diagnostics sao produto |
| 28 | `tests/agent_cli_tests.rs` | regiao evidencia.agent.cli-spec, 31 #[test] | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 29 | `tests/agent_limits_tests.rs` | regiao evidencia.agent.limits, 19 #[test] | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 30 | `tests/agent_runner_tests.rs` | regiao evidencia.agent.runner, 56 #[test] | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 31 | `src/tooling.rs` | tooling.f1.bundle-identity (linhas 881-899), fn bundle_identity | FORJA_ORGANIZATIONAL | UNCHANGED_SINCE_GEN1 | remove |
| 32 | `src/tooling.rs` | tooling.f1.doctor (35-258) — pink doctor | MIXED | UNCHANGED_SINCE_GEN1 | split — manter; trocar apenas recommended_action, que hoje devolve 'scripts/pink-baseline build ... && sudo scripts/pink-baseline publish' quando o binario e incompativel |
| 33 | `src/tooling.rs` | tooling.f1.impact (260-502) | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 34 | `src/tooling.rs` | tooling.f1.freeze-import (504-684) | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 35 | `src/tooling.rs` | tooling.f1.unified-preflight (686-879) | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 36 | `tests/f1_tooling_tests.rs` | regiao evidencia.tooling.f1.contracts, 16 #[test] | MIXED | UNCHANGED_SINCE_GEN1 | split — remover os 2 testes de scripts/pink-baseline (baseline_script_exige_release_identidade_e_publicacao_atomica e o que executa o script); manter os 14 de doctor/impact/preflight/freeze-import |
| 37 | `src/automation/**` | 12 regioes automation.* (mod, compare, plan, fsio, path, root, report) | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 38 | `tests/automation_core_tests.rs` | assercao de fronteira: automation so alcanca crate::agent::sha256_hex | MIXED | UNCHANGED_SINCE_GEN1 | split — a assercao precisa passar a nomear pinker_sha256_contract quando agent.rs sair |
| 39 | `scripts/pinker-cleanup.sh` | script inteiro (363 linhas) | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 40 | `scripts/pinker-flake-runner.sh` | script inteiro (1345 linhas) + tests/pinker_flake_runner_tests.rs | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 41 | `tests/cli_discovery_tests.rs` | 5 assercoes que exigem 'agente' na descoberta da CLI | MIXED | UNCHANGED_SINCE_GEN1 | split — remover as assercoes de 'agente' junto com o subcomando |
| 42 | `tests/nav_cartography_tests.rs` | assercoes sobre pinker_v0::agent::CONTRACT_* e assert_eq!(index.regions.len(), 621) | MIXED | CHANGED_BY_#551 | split — as assercoes de contrato saem com agent.rs; a contagem de regioes e derivada |
| 43 | `docs/development/pink-agent.md, pink-agent-roadmap.md, pink-agent-v1-contract.md, pink-agent-v1-closure.md` | 4 documentos canonicos do pink agente | MIXED | UNCHANGED_SINCE_GEN1 | split — o contrato/roadmap que ENSINA o agente como subsistema Pinker sai ou vira historico explicito; o registro de fechamento e evidencia historica e permanece |
| 44 | `docs/development/consolidated-project-state-contract.md` | secoes sobre o dominio agent de pink estado | MIXED | UNCHANGED_SINCE_GEN1 | split — retirar o dominio agent do contrato quando ele sair do codigo |
| 45 | `.pinker/projections/onda-pink-agente-a..d.toml + recipes/normalizacao-corrente-para-historico.toml` | 4 snapshots FROZEN + 26 regras que nomeiam chaves development.agent.*/evidencia.agent.*; a receita tem override-region para development.agent.artifacts e development.agent.lifecycle | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep — nao recalibrar, nao remover |
| 46 | `src/union_canon.rs` | linha 367 | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 47 | `src/automation/plan.rs` | linha 115 | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 48 | `tests/part_b1_identidade_resultado_tests.rs` | linha 13 | PINKER_PRODUCT | UNCHANGED_SINCE_GEN1 | keep |
| 49 | `agentes/<slot>/ (localizacao fisica dos Task roots dentro do checkout do produto)` | topologia, nao arquivo versionado | UNCERTAIN | UNCHANGED_SINCE_GEN1 | uncertain — preservado, nao decidido aqui |
