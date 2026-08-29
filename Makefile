SHELL := /bin/bash

.PHONY: preflight build check test fmt-check clippy doc-check guard change-history-check ci run-example check-example audit-example smoke docs-sync docs-check nav-sync nav-check cleanup-native forja-check forja-paths-check forja-install-check

CI_ENV := ./ci_env.sh

EX ?= examples/principal_valida.pink

preflight:
	$(CI_ENV) --preflight

build:
	$(CI_ENV) cargo build --locked

check:
	$(CI_ENV) cargo check --locked

test:
	$(CI_ENV) cargo build --locked -p pinker_rt
	$(CI_ENV) cargo test --locked

fmt-check:
	$(CI_ENV) cargo fmt --check

clippy:
	$(CI_ENV) cargo clippy --all-targets --all-features -- -D warnings

doc-check:
	RUSTDOCFLAGS="-D warnings" $(CI_ENV) cargo doc --locked --no-deps

guard:
	$(CI_ENV) cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .

change-history-check:
	$(CI_ENV) cargo test --locked --test change_history_coverage_tests

# Trama Pinker — catálogo documental (Etapa 2).
# `sync` é executado pelo agente/desenvolvedor; `check` roda no CI e não corrige.
docs-sync:
	$(CI_ENV) cargo run --bin pink -- doc sincronizar

docs-check:
	$(CI_ENV) cargo run --bin pink -- doc verificar

# Trama Pinker — navegação do código (Etapa 3).
nav-sync:
	$(CI_ENV) cargo run --bin pink -- nav sincronizar

nav-check:
	$(CI_ENV) cargo run --bin pink -- nav verificar

# Inspeção conservadora; remoção exige `scripts/pinker-cleanup.sh --apply` explícito.
cleanup-native:
	$(CI_ENV) scripts/pinker-cleanup.sh --dry-run

# Forja — autoridade de layout `agentes/`. Roda sem host, sem root e sem rede:
# a suíte monta o layout inteiro num diretório temporário.
forja-check:
	python3 -m unittest discover -s tests/forja -p 'test_*.py' -v

# Nenhum arquivo versionado pode ensinar o layout aposentado da Forja.
forja-paths-check:
	bash scripts/forja/verificar-paths.sh

# Paridade fonte/instalado. Exige o host da Forja, portanto NÃO entra no `ci`
# do repositório: é gate de HOST_E2E.
forja-install-check:
	bash scripts/forja/instalar.sh --check

ci: preflight build check test fmt-check clippy doc-check guard docs-check nav-check change-history-check forja-check forja-paths-check

run-example:
	$(CI_ENV) cargo run --bin pink -- $(EX)

check-example:
	$(CI_ENV) cargo run --bin pink -- --check $(EX)

audit-example:
	$(CI_ENV) cargo run --bin pink -- --tokens $(EX)
	$(CI_ENV) cargo run --bin pink -- --ast $(EX)
	$(CI_ENV) cargo run --bin pink -- --json-ast $(EX)
	$(CI_ENV) cargo run --bin pink -- --check $(EX)
	$(CI_ENV) cargo run --bin pink -- --ir $(EX)
	$(CI_ENV) cargo run --bin pink -- --cfg-ir $(EX)
	$(CI_ENV) cargo run --bin pink -- --selected $(EX)
	$(CI_ENV) cargo run --bin pink -- --machine $(EX)
	$(CI_ENV) cargo run --bin pink -- --pseudo-asm $(EX)
	$(CI_ENV) cargo run --bin pink -- --asm-s $(EX)

smoke:
	$(CI_ENV) cargo run --bin pink -- --check examples/principal_valida.pink
	$(CI_ENV) cargo run --bin pink -- --run examples/run_soma.pink
