SHELL := /bin/bash

.PHONY: preflight build test fmt-check clippy guard ci run-example check-example audit-example smoke docs-sync docs-check

CI_ENV := ./ci_env.sh

EX ?= examples/principal_valida.pink

preflight:
	$(CI_ENV) --preflight

build:
	$(CI_ENV) cargo build --locked

test:
	$(CI_ENV) cargo test --locked

fmt-check:
	$(CI_ENV) cargo fmt --check

clippy:
	$(CI_ENV) cargo clippy --all-targets --all-features -- -D warnings

guard:
	$(CI_ENV) cargo run --bin pink -- --run apps/guardiao_pinker/principal.pink -- --repo .

# Trama Pinker — catálogo documental (Etapa 2).
# `sync` é executado pelo agente/desenvolvedor; `check` roda no CI e não corrige.
docs-sync:
	$(CI_ENV) cargo run --bin pink -- doc sincronizar

docs-check:
	$(CI_ENV) cargo run --bin pink -- doc verificar

ci: preflight build test fmt-check clippy guard docs-check

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
