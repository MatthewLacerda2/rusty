# rusty's gates, runnable in one command (#485).
#
#   make gates       everything CI blocks on, fastest-failing first
#   make check       the fast edit loop
#   make pre-commit  what the commit hook runs (formatting + size gate, staged files)
#   make setup       once per clone: point git at the committed hooks in .githooks/,
#                    and switch the compile cache on when sccache is installed
#
# Portable to GNU make 3.81 (what macOS ships): no .ONESHELL, no $(file), no ::=.
# A target documented `## [gate]` must be listed in GATES and vice versa —
# `make inventory` enforces it, so the gate list cannot drift from its targets.

LINT := cargo run --quiet --manifest-path tools/lint/Cargo.toml
# The test runner (#484): cargo-nextest, configured in .config/nextest.toml.
# nextest does not run doctests, so the `test` gate runs `cargo test --doc` beside it.
TEST := cargo nextest run --locked

# The gates, in the order `make gates` runs them. `deny` sits last: its failures
# are about dependencies (and it needs the network), so a code failure reports first.
GATES := fmt size determinism direction components parity test-lint scripts clippy doc test deny
# Checks on the gate runner itself, run before any gate.
SELF_CHECKS := target-dir inventory

.DEFAULT_GOAL := help
.NOTPARALLEL:
.PHONY: help setup compile-cache check gates pre-commit $(SELF_CHECKS) $(GATES) mergeable queue blockers

help: ## List the verbs
	@grep -E '^[a-z-]+:.*## ' $(firstword $(MAKEFILE_LIST)) | \
		awk -F':.*## ' '{ printf "  %-14s %s\n", $$1, $$2 }'

setup: ## Once per clone: the committed git hooks, and the compile cache if sccache is installed
	git config core.hooksPath .githooks
	@echo "setup: pre-commit hook active (formatting + size gate)"
	@$(MAKE) --no-print-directory compile-cache

# The compile cache (#492). sccache stores each dependency compile under a hash of
# its inputs, so a fresh worktree reuses what an earlier one built instead of
# cold-compiling ~450 crates. rusty's own crate compiles incrementally, which
# sccache passes through uncached: the edit loop is unchanged. The config goes in
# the gitignored worktrees folder (cargo reads config from parent directories), so
# it reaches every worktree and nothing else: not the user's other projects, not
# CI. Machines without sccache get nothing written. The path is absolute because
# cargo did not find a bare `sccache` from a config file. docs/testing.md has the numbers.
CACHE_CONFIG = $(shell dirname "$$(git rev-parse --path-format=absolute --git-common-dir)")/.claude/worktrees/.cargo/config.toml
CACHE_SIZE = 2G

compile-cache: ## Switch the worktrees' compile cache on (needs sccache); rerun after installing it
	@sccache="$$(command -v sccache)"; \
	if [ -z "$$sccache" ]; then \
		echo "compile-cache: off, sccache is not installed (cargo install --locked sccache, then make setup)"; \
		exit 0; \
	fi; \
	mkdir -p "$(dir $(CACHE_CONFIG))" && \
	printf '%s\n' \
		'# Written by `make setup` (#492): the worktrees share a compile cache.' \
		'# Delete this file to turn it off. docs/testing.md explains it.' \
		'[build]' "rustc-wrapper = \"$$sccache\"" '' \
		'[env]' '# Read when the sccache server starts; it evicts the oldest entries past this.' \
		'SCCACHE_CACHE_SIZE = "$(CACHE_SIZE)"' > "$(CACHE_CONFIG)" && \
	echo "compile-cache: on for every worktree ($(CACHE_CONFIG), cap $(CACHE_SIZE))"

gates: $(SELF_CHECKS) $(GATES) ## Everything CI blocks on, fastest-failing first
	@echo "gates: all green"

check: target-dir fmt size ## Fast edit loop: formatting, size gate, dev-feature clippy
	cargo clippy --all-targets --features dev

pre-commit: ## What the commit hook runs: formatting + size gate on staged .rs files
	@cargo fmt --all --check
	@files="$$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs')"; \
		if [ -n "$$files" ]; then $(LINT) -- $$files; fi

# ---- self-checks ----------------------------------------------------------

# A target dir shared between worktrees lets one branch's artifacts stand in for
# another's, so a green gate stops meaning *this* branch compiled. Cargo's own
# resolution (env vars and every config file) is the authority, so ask it.
target-dir: ## Refuse to run when the cargo target dir is outside this worktree
	@dir="$$(cargo metadata --no-deps --format-version 1 | \
		sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"; \
	[ -n "$$dir" ] || { echo "target-dir: cargo could not report its target dir (see above)"; exit 1; }; \
	if [ -d "$$dir" ]; then dir="$$(cd "$$dir" && pwd -P)"; fi; \
	root="$$(pwd -P)"; inside=no; \
	case "$$dir/" in "$$root/"*|"$(CURDIR)/"*) inside=yes ;; esac; \
	case "$$dir/" in */../*) inside=no ;; esac; \
	if [ "$$inside" != yes ]; then \
		echo "target-dir: cargo builds into $$dir, outside this worktree ($$root)."; \
		echo "  A shared target dir is a false green. Unset CARGO_TARGET_DIR / CARGO_BUILD_TARGET_DIR"; \
		echo "  (or build.target-dir in a cargo config)."; exit 1; \
	fi

inventory: ## The gate targets and the GATES list name the same set
	@documented="$$(sed -n 's/^\([a-z-]*\):.*## \[gate\].*/\1/p' $(firstword $(MAKEFILE_LIST)) | sort)"; \
	listed="$$(for g in $(GATES); do echo $$g; done | sort)"; \
	if [ "$$documented" != "$$listed" ]; then \
		echo "inventory: the GATES list and the \`## [gate]\` targets disagree."; \
		echo "  documented: " $$documented; echo "  listed:     " $$listed; exit 1; \
	fi

# ---- the gates (each mirrors a CI step; .github/workflows/{ci,lint}.yml) ----

fmt: ## [gate] rustfmt, engine and tools/lint
	cargo fmt --all --check
	cargo fmt --manifest-path tools/lint/Cargo.toml --check

size: ## [gate] File-length cap, full scan (tools/lint)
	$(LINT)

determinism: ## [gate] No wall-clock / unseeded RNG in the sim modules
	$(LINT) -- --determinism

direction: ## [gate] Sim modules never import render/editor/wgpu/egui
	$(LINT) -- --direction

components: ## [gate] Every first-class component has all four axes
	$(LINT) -- --components

parity: ## [gate] Migrated inspector cards route through scene::authoring
	$(LINT) -- --parity

test-lint: ## [gate] tools/lint's own tests
	cargo test --manifest-path tools/lint/Cargo.toml --locked

clippy: ## [gate] Clippy: default, dev, and the no-editor player build
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --features dev -- -D warnings
	cargo clippy --all-targets --no-default-features -- -D warnings

# No separate build gate: `cargo test` compiles every binary its feature set
# enables, exactly as CI relies on (#482).
doc: ## [gate] Rustdoc with warnings denied, both feature sets
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked --features dev

test: ## [gate] Engine tests (nextest) and doctests, both feature sets
	@command -v cargo-nextest >/dev/null 2>&1 || { \
		echo "test: cargo-nextest is not installed — cargo install --locked cargo-nextest"; exit 1; }
	$(TEST)
	$(TEST) --features dev
	cargo test --doc --locked
	cargo test --doc --locked --features dev

# The git CLI fetches the advisory DB: cargo-deny's built-in fetcher can't get
# through the cloud sessions' proxy, and the CLI works everywhere else too.
deny: ## [gate] Advisories, bans, sources, licenses (cargo-deny)
	@command -v cargo-deny >/dev/null 2>&1 || { \
		echo "deny: cargo-deny is not installed — cargo install --locked cargo-deny"; exit 1; }
	cargo deny check advisories bans sources licenses

# ---- merging (#486) --------------------------------------------------------

# The merge helpers' own tests: stdlib `unittest`, no build, well under a
# second — and a gate, because `mergeable` is the only thing between a red PR
# and `main` while it has no required checks (#491).
scripts: ## [gate] Tests of the merge helpers in .github/scripts
	python3 -m unittest discover --start-directory .github/scripts/tests --quiet

# Did CI really run, and pass, on this PR's head commit? Both workflows' gates,
# a gated job that genuinely ran, and no red run beside a green one. The last
# check before any merge; the script's docstring has the incidents behind it.
mergeable: ## Did CI really run on this PR's head? make mergeable PR=524
	@test -n "$(PR)" || { echo "mergeable: which pull request? e.g. make mergeable PR=524" >&2; exit 1; }
	@python3 .github/scripts/mergeable.py $(PR)

# The same question in a loop, with the rebase and the waiting done for you.
# Serialized: one branch at a time, in the order given. Compile-checks each
# rebased head before pushing it (#571; ARGS=--no-check skips). Never resolves
# a conflict or touches a worktree of yours; it names what to clean up.
# ARGS passes flags through, e.g. ARGS=--dry-run (reads only, writes nothing).
queue: ## Rebase, wait for CI, squash-merge each in turn. make queue PRS="524 526", or ARGS=--watch
	@test -n "$(PRS)$(filter --watch,$(ARGS))" || { echo "queue: which pull requests? e.g. make queue PRS=\"524 526\", or make queue ARGS=--watch" >&2; exit 1; }
	@python3 .github/scripts/merge-queue.py $(PRS) $(ARGS)

# "Blocked by #N" written in an issue body but never recorded as a GitHub
# relationship is invisible to everything that plans from the graph (#624).
# Lists them and exits 1; ARGS=--fix records them. Run at the start of a batch.
blockers: ## Prose-only "Blocked by #N" GitHub never recorded. ARGS=--fix records them
	@python3 .github/scripts/blockers.py $(ARGS)
