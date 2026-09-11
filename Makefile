# Makefile for common tasks in a Rust project
# Detect current branch
CURRENT_BRANCH := $(shell git rev-parse --abbrev-ref HEAD)
ZIP_NAME = OptionStratLib.zip


# Default target
.PHONY: all
all: test fmt lint build

# Build the project
.PHONY: build
build:
	cargo build

.PHONY: release
release:
	cargo build --release

# Run tests
.PHONY: test
test:
	LOGLEVEL=WARN cargo test

# Format the code
.PHONY: fmt
fmt:
	cargo +stable fmt --all

# Check formatting
.PHONY: fmt-check
fmt-check:
	cargo +stable fmt --check

# Run Clippy for linting
.PHONY: lint
lint:
	cargo clippy --all-targets --all-features -- -D warnings

.PHONY: lint-fix
lint-fix: 
	cargo clippy --fix --all-targets --all-features --allow-dirty --allow-staged --workspace -- -D warnings

# Clean the project
.PHONY: clean
clean:
	cargo clean

# Pre-push checks
.PHONY: check
check: test fmt-check lint

# Run the project
.PHONY: run
run:
	cargo run

.PHONY: fix
fix:
	cargo fix --allow-staged --allow-dirty

.PHONY: pre-push
pre-push: fix fmt lint-fix test readme doc check-features

# Build, lint and test every supported feature combination for the `ig-client`
# package (scoped with -p so the workspace example crates, which depend on
# default features, are not dragged into --no-default-features builds), then
# assert that lightstreamer-rs (GPL-3.0-only) and sqlx stay out of the
# --no-default-features dependency graph.
.PHONY: check-features
check-features:
	@for features in "--no-default-features" "--no-default-features --features persistence" "--no-default-features --features streaming" "--all-features"; do \
		echo "==> cargo build -p ig-client $$features"; \
		cargo build -p ig-client $$features || exit 1; \
		echo "==> cargo clippy -p ig-client --all-targets $$features -- -D warnings"; \
		cargo clippy -p ig-client --all-targets $$features -- -D warnings || exit 1; \
		echo "==> cargo test -p ig-client $$features"; \
		cargo test -p ig-client $$features || exit 1; \
		echo "==> cargo doc -p ig-client --no-deps $$features"; \
		RUSTDOCFLAGS="-D warnings" cargo doc -p ig-client --no-deps $$features || exit 1; \
	done
	@echo "==> dependency-graph assertions"
	@FAILED=0; \
	check() { \
		TREE="$$(cargo tree -p ig-client $$1 -e normal)" || exit 1; \
		if echo "$$TREE" | grep -q "$$2"; then FOUND=present; else FOUND=absent; fi; \
		if [ "$$FOUND" != "$$3" ]; then \
			echo "$$2 is $$FOUND in the '$$1' dependency graph, expected $$3"; \
			FAILED=1; \
		else \
			echo "ok: $$2 $$FOUND with '$$1'"; \
		fi; \
	}; \
	check "--no-default-features" lightstreamer-rs absent; \
	check "--no-default-features --features persistence" lightstreamer-rs absent; \
	check "--no-default-features --features streaming" lightstreamer-rs present; \
	check "--no-default-features" sqlx absent; \
	check "--no-default-features --features streaming" sqlx absent; \
	check "--no-default-features --features persistence" sqlx present; \
	exit $$FAILED

.PHONY: doc
doc:
	cargo clippy -- -W missing-docs

.PHONY: doc-open
doc-open:
	cargo doc --open

.PHONY: publish
publish: readme
	cargo package
	cargo publish

.PHONY: coverage
coverage:
	export LOGLEVEL=WARN
	cargo install cargo-tarpaulin
	mkdir -p coverage
	cargo tarpaulin --verbose --all-features --workspace --timeout 0 --out Xml --output-dir coverage

.PHONY: coverage-html
coverage-html:
	export LOGLEVEL=WARN
	cargo install cargo-tarpaulin
	mkdir -p coverage
	cargo tarpaulin --color Always --engine llvm --tests --all-targets --all-features --workspace --timeout 0 --out Html --output-dir coverage

.PHONY: open-coverage
open-coverage:
	open tarpaulin-report.html

# Rule to show git log
git-log:
	@if [ "$(CURRENT_BRANCH)" = "HEAD" ]; then \
		echo "You are in a detached HEAD state. Please check out a branch."; \
		exit 1; \
	fi; \
	echo "Showing git log for branch $(CURRENT_BRANCH) against main:"; \
	git log main..$(CURRENT_BRANCH) --pretty=full

.PHONY: create-doc
create-doc:
	cargo doc --no-deps --document-private-items

.PHONY: readme
readme: check-cargo-readme create-doc
	cargo readme > README.md

.PHONY: check-cargo-readme
check-cargo-readme:
	@command -v cargo-readme > /dev/null || (echo "Installing cargo-readme..."; cargo install cargo-readme)

.PHONY: check-spanish
check-spanish:
	@rg -n --pcre2 -e '^\s*(//|///|//!|#|/\*|\*).*?[áéíóúÁÉÍÓÚñÑ¿¡]' \
    	    --glob '!target/*' \
    	    --glob '!**/*.png' \
    	    . || (echo "❌  Spanish comments found"; exit 1)

.PHONY: zip
zip:
	@echo "Creating $(ZIP_NAME) without any 'target' directories, 'Cargo.lock', and hidden files..."
	@find . -type f \
		! -path "*/target/*" \
		! -path "./.*" \
		! -name "Cargo.lock" \
		! -name ".*" \
		| zip -@ $(ZIP_NAME)
	@echo "$(ZIP_NAME) created successfully."


.PHONY: check-cargo-criterion
check-cargo-criterion:
	@command -v cargo-criterion > /dev/null || (echo "Installing cargo-criterion..."; cargo install cargo-criterion)

.PHONY: bench
bench: check-cargo-criterion
	cargo criterion --output-format=quiet

.PHONY: bench-show
bench-show:
	open target/criterion/report/index.html

.PHONY: bench-save
bench-save: check-cargo-criterion
	cargo criterion --output-format quiet --history-id v0.3.2 --history-description "Version 0.3.2 baseline"

.PHONY: bench-compare
bench-compare: check-cargo-criterion
	cargo criterion --output-format verbose

.PHONY: bench-json
bench-json: check-cargo-criterion
	cargo criterion --message-format json

.PHONY: bench-clean
bench-clean:
	rm -rf target/criterion


.PHONY: workflow-coverage
workflow-coverage:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job code_coverage_report \
       -P ubuntu-latest=catthehacker/ubuntu:latest \
       --privileged

.PHONY: workflow-build
workflow-build:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job build \
       -P ubuntu-latest=catthehacker/ubuntu:latest

.PHONY: workflow-lint
workflow-lint:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job lint

.PHONY: workflow-test
workflow-test:
	DOCKER_HOST="$${DOCKER_HOST}" act push --job run_tests

.PHONY: workflow
workflow: workflow-build workflow-lint workflow-test workflow-coverage

.PHONY: generate_markdown
generate_markdown:
	./doc/generate_md_docs.sh
