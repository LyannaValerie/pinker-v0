SHELL := /bin/bash

.PHONY: build test fmt-check clippy ci run-example check-example audit-example smoke

EX ?= examples/principal_valida.pink

build:
	bash -lc 'cargo build --locked'

test:
	bash -lc 'cargo test --locked'

fmt-check:
	bash -lc 'cargo fmt --check'

clippy:
	bash -lc 'cargo clippy --all-targets --all-features -- -D warnings'

ci: build test fmt-check clippy

run-example:
	bash -lc 'cargo run --bin pink -- $(EX)'

check-example:
	bash -lc 'cargo run --bin pink -- --check $(EX)'

audit-example:
	bash -lc 'cargo run --bin pink -- --tokens $(EX)'
	bash -lc 'cargo run --bin pink -- --ast $(EX)'
	bash -lc 'cargo run --bin pink -- --json-ast $(EX)'
	bash -lc 'cargo run --bin pink -- --check $(EX)'
	bash -lc 'cargo run --bin pink -- --ir $(EX)'
	bash -lc 'cargo run --bin pink -- --cfg-ir $(EX)'
	bash -lc 'cargo run --bin pink -- --selected $(EX)'
	bash -lc 'cargo run --bin pink -- --machine $(EX)'
	bash -lc 'cargo run --bin pink -- --pseudo-asm $(EX)'
	bash -lc 'cargo run --bin pink -- --asm-s $(EX)'

smoke:
	bash -lc 'cargo run --bin pink -- --check examples/principal_valida.pink'
	bash -lc 'cargo run --bin pink -- --run examples/run_soma.pink'
