# rusty's gates, runnable in one command (#485).
#
#   make gates       everything CI blocks on, fastest-failing first
#   make quick-gates the seconds-long gates and self-checks (the cloud split, #1179)
#   make check       the fast edit loop
#   make pre-commit  what the commit hook runs (formatting + size gate, staged files)
#   make resolved    the push hook's check (rustfmt on both crates, tools/lint included)
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
# The gates' fixed proptest seed (#1852): ci.yml's, read from there so there is one
# number, and a local gate run goes red only for the branch under test, never for a
# seed that drew an old bug. Exported to every recipe, so `make test` and `make
# test-one-process` both run on it; only `make test-explore` draws fresh seeds. An
# exported PROPTEST_RNG_SEED wins, so a seed from a report still reproduces.
# test_workflow_hygiene.py runs both verbs' recipes on a probe and fails when either
# sees another number than ci.yml's, or none.
PROPTEST_SEED := $(shell sed -n "s/^  PROPTEST_RNG_SEED: .* '\([0-9][0-9]*\)' .*/\1/p" .github/workflows/ci.yml)
export PROPTEST_RNG_SEED ?= $(PROPTEST_SEED)

# The gates, in the order `make gates` runs them. The quick ones take seconds; the
# heavy ones compile, so a cold run passes a cloud tool call's ten-minute limit and
# is split (`make quick-gates`, then each heavy gate; cloud-brief.md, *The container*).
# `deny` sits last: its failures are about dependencies (and it needs the network),
# so a code failure reports first. GATES is exactly the two lists joined —
# `make scripts` (test_gate_mirror.py) fails when it is anything else.
QUICK_GATES := fmt size rules layers determinism direction components parity test-lint scripts feature-split docrefs
HEAVY_GATES := clippy doc test deny
GATES := $(QUICK_GATES) $(HEAVY_GATES)
# Checks on the gate runner itself, run before any gate.
SELF_CHECKS := target-dir inventory

.DEFAULT_GOAL := help
.NOTPARALLEL:
.PHONY: help setup compile-cache check gates quick-gates pre-commit resolved $(SELF_CHECKS) $(GATES) mergeable ruleset queue queue-daemon queue-stop queue-status queue-events blockers checks conflicts reclaim coder review-level rebriefs mutants-remote move-crate test-explore test-one-process dead-pub editor-capture game game-test export launcher bench bless-converters

help: ## List the verbs
	@grep -E '^[a-z-]+:.*## ' $(firstword $(MAKEFILE_LIST)) | \
		awk -F':.*## ' '{ printf "  %-14s %s\n", $$1, $$2 }'

setup: ## Once per clone: the committed git hooks, and the compile cache if sccache is installed
	git config core.hooksPath .githooks
	@echo "setup: pre-commit (formatting + size gate) and pre-push (formatting) hooks active"
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

quick-gates: $(SELF_CHECKS) $(QUICK_GATES) ## The seconds-long gates and self-checks (all but HEAVY_GATES)
	@echo "quick-gates: all green"

check: target-dir fmt size ## Fast edit loop: formatting, size gate, dev-feature clippy
	cargo clippy --all-targets --features dev

# tools/lint is its own crate, out of `--all`'s reach: formatted only when staged (#1096).
pre-commit: ## What the commit hook runs: formatting + size gate on staged .rs, .py, .lua, .wgsl and .md files
	@cargo fmt --all --check
	@if git diff --cached --name-only --diff-filter=ACMR -- tools/lint | grep -q .; then \
		cargo fmt --manifest-path tools/lint/Cargo.toml --check; fi
	@files="$$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs' '*.py' '*.lua' '*.wgsl' '*.md')"; \
		if [ -n "$$files" ]; then $(LINT) -- $$files; fi

# `git rebase --continue` and `--no-verify` skip the commit hook; the push hook covers them (#1096).
resolved: ## The push hook's check: rustfmt on the root crate and tools/lint, as lint.yml runs it
	@python3 .github/scripts/resolved.py

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
# `make scripts` holds the mirror: test_gate_mirror.py fails when a command below
# is missing from the workflows, or a clippy run adds more than `-D warnings` (#964).

fmt: ## [gate] rustfmt, engine and tools/lint
	cargo fmt --all --check
	cargo fmt --manifest-path tools/lint/Cargo.toml --check

size: ## [gate] File-length cap on .rs, .py, .lua and .wgsl, and the agent docs' shape (CLAUDE.md, skills), full scan (tools/lint)
	$(LINT)

rules: ## [gate] Finding-based rules with inline allows, one module each in tools/lint/src/rules (docs/linting.md lists them)
	$(LINT) -- --rules

layers: ## [gate] Module imports and external crates match the declared layer table (tools/lint/src/layers/table.rs)
	$(LINT) -- --layers

determinism: ## [gate] Lint policy: only platform roots opt out of the clock/RNG ban; locked escapes; reasons denied
	$(LINT) -- --determinism

direction: ## [gate] Sim modules never import render/editor/wgpu/egui
	$(LINT) -- --direction

components: ## [gate] Every first-class component has all four axes
	$(LINT) -- --components

parity: ## [gate] Migrated inspector cards route through scene::authoring
	$(LINT) -- --parity

test-lint: ## [gate] tools/lint's own tests
	cargo test --manifest-path tools/lint/Cargo.toml --locked

# A dev-dependency that turns on a feature the normal build lacks makes `cargo test`
# rebuild that crate and everything above it (#552). No build: `cargo tree` twice.
feature-split: ## [gate] No dev-dependency turns on a feature the normal build lacks (docs/testing.md)
	python3 .github/scripts/feature_split.py

# A path, make verb or API namespace the docs name must exist (#973): agents drive
# rusty by its docs, and a move that leaves a reference behind sends them searching.
docrefs: ## [gate] Paths, make verbs and namespaces written in the docs exist
	python3 .github/scripts/docrefs.py

clippy: ## [gate] Clippy: default, dev, the no-editor player build, and tools/lint
	cargo clippy --all-targets -- -D warnings
	cargo clippy --all-targets --features dev -- -D warnings
	cargo clippy --all-targets --no-default-features -- -D warnings
	cargo clippy --manifest-path tools/lint/Cargo.toml --all-targets --locked -- -D warnings

# No separate build gate: `cargo test` compiles every binary its feature set
# enables, exactly as CI relies on (#482).
doc: ## [gate] Rustdoc with warnings denied, both feature sets
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked --features dev

test: ## [gate] Engine tests (nextest) and doctests, both feature sets, on the gates' proptest seed
	@command -v cargo-nextest >/dev/null 2>&1 || { \
		echo "test: cargo-nextest is not installed — cargo install --locked cargo-nextest"; exit 1; }
	$(TEST)
	$(TEST) --features dev
	cargo test --doc --locked
	cargo test --doc --locked --features dev

# Fresh proptest seeds, as the nightly draws (#1852): a signal, never a gate. A case
# it finds is written to proptest-regressions/ and committed with its fix
# (docs/testing.md). PROPTEST_CASES=<n> runs more cases per property.
# Both feature sets run even when the first fails, and the exit says if either did.
test-explore: ## Signal: the nextest suite, both feature sets, on random proptest seeds
	@command -v cargo-nextest >/dev/null 2>&1 || { \
		echo "test-explore: cargo-nextest is not installed — cargo install --locked cargo-nextest"; exit 1; }
	@rc=0; \
	env -u PROPTEST_RNG_SEED $(TEST) || rc=1; \
	env -u PROPTEST_RNG_SEED $(TEST) --features dev || rc=1; \
	exit $$rc

# The converters' fingerprints (#1286): re-blessed after a bump of a converter's
# `version`, never in place of one (the test refuses). Blender's rows only where
# Blender runs (RUSTY_BLENDER, or `blender` on the PATH), as CI's `blender` job does.
bless-converters: ## Re-bless src/asset/convert/fingerprints.txt after a converter's version bump
	RUSTY_BLESS_CONVERTERS=1 $(TEST) -E 'test(converter_fingerprints)'
	@if command -v "$${RUSTY_BLENDER:-blender}" >/dev/null 2>&1; then \
		RUSTY_BLESS_CONVERTERS=1 $(TEST) --run-ignored only -E 'test(converter_fingerprints)'; \
	else echo "bless-converters: no Blender found, its rows left as they are"; fi

# The git CLI fetches the advisory DB: cargo-deny's built-in fetcher can't get
# through the cloud sessions' proxy, and the CLI works everywhere else too.
deny: ## [gate] Advisories, bans, sources, licenses (cargo-deny)
	@command -v cargo-deny >/dev/null 2>&1 || { \
		echo "deny: cargo-deny is not installed — cargo install --locked cargo-deny"; exit 1; }
	cargo deny check advisories bans sources licenses

# ---- merging (#486) --------------------------------------------------------

# The merge helpers' own tests: stdlib `unittest`, no build, well under a
# second — and a gate, because `mergeable` is the merge path's judge: the
# ruleset's required checks (#491) cannot tell a draft's run from a ready one.
scripts: ## [gate] Tests of the merge and mutation helpers in .github/scripts, and of tools/launcher
	python3 -m unittest discover --start-directory .github/scripts/tests --top-level-directory .github/scripts --quiet
	python3 -m unittest discover --start-directory tools/launcher/tests --top-level-directory tools/launcher --quiet

# Did CI really run, and pass, on this PR's head commit? Both workflows' gates,
# a gated job that genuinely ran, and no red run beside a green one. The last
# check before any merge; the script's docstring has the incidents behind it.
mergeable: ## Did CI really run on this PR's head? make mergeable PR=524
	@test -n "$(PR)" || { echo "mergeable: which pull request? e.g. make mergeable PR=524" >&2; exit 1; }
	@python3 .github/scripts/mergeable.py $(PR)

# `main`'s ruleset is code (#491): .github/rulesets/main.json requires `ci-gate`
# and `lint-gate`. Read-only by default, exits 1 on drift; ARGS=--apply writes the
# file to GitHub. Applying changes how the queue merges: do it between merges.
ruleset: ## Does main's live ruleset match .github/rulesets/main.json? ARGS=--apply writes it
	@python3 .github/scripts/ruleset.py $(ARGS)

# The same question in a loop, with the update from main and the waiting done for you.
# Serialized: one branch at a time, in the order given. Merges origin/main into
# each (never a rebase, so never a force-push, #1568) and compile-checks the
# updated head before pushing it (#571; ARGS=--no-check skips the compile, never
# tools/lint's build-free passes on the merged tree, #1956). Never resolves
# a conflict or touches a worktree of yours; it names what to clean up.
# ARGS passes flags through, e.g. ARGS=--dry-run (reads only, writes nothing).
queue: ## Merge main in, wait for CI, squash-merge each in turn. make queue PRS="524 526", or ARGS=--watch
	@test -n "$(PRS)$(filter --watch,$(ARGS))" || { echo "queue: which pull requests? e.g. make queue PRS=\"524 526\", or make queue ARGS=--watch" >&2; exit 1; }
	@python3 .github/scripts/merge-queue.py $(PRS) $(ARGS)

# The watch exits on its deadline, on "nothing left", when its own code changes
# on main, and when GitHub is down; each used to be a relaunch by hand (#908).
# The supervisor runs detached (a background command dies at two hours), moves
# its checkout to origin/main before each watch, and decides whether and when to
# start it again. Run it from a worktree of its own. ARGS pass to the watch.
queue-daemon: ## Keep the merge-queue watch running, detached; log in .git/merge-queue/queue.log
	@python3 .github/scripts/queue-daemon.py --detach $(ARGS)

queue-stop: ## Ask the watch and its supervisor to stop, between pull requests
	@python3 .github/scripts/queue-daemon.py --stop

# The orchestrator's read of the queue (#1876), two fixed commands so the watch
# is never retyped: liveness from the supervisor's worktree lock (its PID, asked
# alive; never a pattern matched against every command line), and the stream a
# `Monitor` runs: the log from 30 lines back, filtered to what someone acts on,
# plus coders' "Standing down" comments. `merging/view/` has the incidents.
queue-status: ## Does the queue's supervisor run (its lock's PID, checked alive)? Plus the log's last line and age, and green PRs passed over for an hour
	@python3 .github/scripts/queue-view.py status

queue-events: ## The stream a Monitor runs: hand-backs, merges, ready flips, stand-downs; exits 1 once the supervisor is gone
	@python3 .github/scripts/queue-view.py events

# "Blocked by #N" written in an issue body but never recorded as a GitHub
# relationship is invisible to everything that plans from the graph (#624).
# Lists them and exits 1; ARGS=--fix records them. Run at the start of a batch.
blockers: ## Prose-only "Blocked by #N" GitHub never recorded. ARGS=--fix records them
	@python3 .github/scripts/blockers.py $(ARGS)

# The PR-on-first-commit rule exists so nothing goes stale unnoticed; this is
# the noticing (#971). Lists branches ahead of main with no PR, merged branches
# still standing, branch names off the convention, assignments older than a day
# with no branch or PR, open issues with no type label, and open bugs with no
# `Kind:` line or one their sub-issue parent contradicts (#1173). A signal: it also
# runs weekly (board.yml, into the run summary) and exits 1 on findings.
.PHONY: board
board: ## Stray branches, stale assignments, unlabelled issues, bugs with no Kind. ARGS="--hours 48" sets the age
	@python3 .github/scripts/board.py $(ARGS)

# Human checks never hold a merge, so after a batch they are scattered across
# its merged PRs (#929). Lists every unticked `- [ ]` under a "For a human"
# heading in the PRs merged since SINCE, and the issues labelled both planning
# and human, as Markdown for the batch report. A report, never a gate: exits 0.
checks: ## Unticked human checks from PRs merged since then. make checks SINCE=12h or SINCE=2026-10-16
	@test -n "$(SINCE)" || { echo "checks: since when? e.g. make checks SINCE=12h or SINCE=2026-10-16T18:00Z" >&2; exit 1; }
	@python3 .github/scripts/human-checks.py '$(SINCE)'

# The record of every conflict with `main` the queue met (#1723): how many, how
# many the sorted-list rule resolved, and what share of the files it could have.
conflicts: ## Conflicts with main the queue met since then. make conflicts SINCE=24h or SINCE=2026-11-02
	@test -n "$(SINCE)" || { echo "conflicts: since when? e.g. make conflicts SINCE=24h or SINCE=2026-11-02" >&2; exit 1; }
	@python3 .github/scripts/conflicts.py '$(SINCE)'

# Each worktree's target/ holds GBs, and removing it by hand per merged branch
# gets skipped under load (#913). Removes every worktree under .claude/worktrees/
# whose PR merged or closed, target/ first; never one with uncommitted changes,
# unpushed commits or a lock, which it names. Prints the rest's target/ sizes and
# free disk before and after; a linked worktree outside .claude/worktrees/ is
# named OUTSIDE, never touched (#982). ARGS=--dry-run lists without deleting.
reclaim: ## Remove merged worktrees (and their target/) and finished local branches, report disk. ARGS=--dry-run lists only
	@python3 .github/scripts/reclaim.py $(ARGS)

# A batch's two dispatch steps, scripted (#1569). `coder` reads the newest
# `Brief` comment on the issue and creates the one-off routine that starts the
# cloud coder (through `claude mcp serve`, so the
# claude.ai token stays inside claude); running it again is the re-brief launch.
# It refuses a closed, `planning` or `human` issue, and a sixth running coder
# (#1856; a re-brief never counts against itself, ARGS=--over-limit overrides).
# ARGS=--dry-run prints the routine and creates nothing. `review-level` prints
# `high` or `medium` for an independent review, and the paths that decided it.
coder: ## Start the cloud coder on an issue's newest Brief. make coder ISSUE=1569, ARGS=--dry-run creates nothing
	@test -n "$(ISSUE)" || { echo "coder: which issue? e.g. make coder ISSUE=1569" >&2; exit 1; }
	@python3 .github/scripts/coder.py $(ISSUE) $(ARGS)

review-level: ## The independent review's level (high or medium) and why. make review-level PR=1570
	@test -n "$(PR)" || { echo "review-level: which pull request? e.g. make review-level PR=1570" >&2; exit 1; }
	@python3 .github/scripts/review-level.py $(PR)

# Over half of a batch's launches were re-briefs, with no record of why (#1724).
# Counts the `Brief` comments: briefs per issue, issues done in one, and
# re-briefs by their `Round:` line. A report, never a gate: exits 0.
rebriefs: ## Briefs per issue and re-briefs by Round. ARGS="--since 2026-10-29" sets the window
	@python3 .github/scripts/rebriefs.py $(ARGS)

# ---- signals (#750)---------------------------------------------------------

# Mutation testing on GitHub's runners, over exactly what SCOPE names: `diff`
# (this branch against origin/main) or path globs. Dispatches
# mutants-on-request.yml on the pushed branch, waits, and prints the report and
# the survivors' diffs. A signal, never a gate: survivors exit 0, and only "no
# report" (a red, cancelled or missing run) exits 1. No PR runs mutation any
# more, so ask for this before readying a branch that adds mechanism.
# The mechanical half of moving a module into its own crate (crates/CLAUDE.md).
move-crate: ## Move a module into crates/<m>/: make move-crate MODULE=<m>, then fix what it lists
	@test -n "$(MODULE)" || { echo "move-crate: name the module: make move-crate MODULE=<m>" >&2; exit 2; }
	@python3 .github/scripts/move_crate.py $(MODULE)

mutants-remote: ## Mutation on GitHub's runners, scoped. make mutants-remote SCOPE=diff or SCOPE='src/physics/**'
	@test -n "$(SCOPE)" || { \
		echo "mutants-remote: what should be mutated? SCOPE=diff, or path globs" >&2; \
		echo "                e.g. make mutants-remote SCOPE='src/navigation/**'" >&2; \
		exit 1; }
	@python3 .github/scripts/mutants-remote.py '$(SCOPE)'

# The `dev` suite under plain `cargo test`, every test of a binary in one process
# (#1338): nextest's process per test hides a test that leaks process-wide state (the
# working directory, a `static` left set); while cargo-mutants ran `cargo test`, such a
# leak broke the mutation baseline of every branch after it (#1042, #1069, #1337, #1844).
# A signal, never a gate: it runs nightly on `main` in cargo-test.yml, and this verb
# runs it on demand. `NEXTEST=1` keeps the guarantee that a test never holds a provider
# key (`dev::providers::credentials`, #974), and it runs on the gates' proptest seed
# (PROPTEST_RNG_SEED, above; the nightly explores). `--quiet`, since its ~1,800 pass lines are
# output every agent pays for; `--no-fail-fast` runs every binary, and the failing tests
# are named again at the end (the log stays in target/one-process.log).
test-one-process: ## Signal: the dev suite under plain cargo test, one process per binary
	@mkdir -p target; \
	{ NEXTEST=1 cargo test --locked --features dev --tests --no-fail-fast --quiet 2>&1; \
		echo $$? > target/one-process.rc; } | tee target/one-process.log; \
	rc=$$(cat target/one-process.rc); \
	if [ "$$rc" -ne 0 ]; then \
		printf '\nFailed under one process (#1338):\n'; \
		sed -n 's/^---- \(.*\) stdout ----$$/\1/p' target/one-process.log | sort -u; \
	fi; \
	exit "$$rc"

# `pub` items nothing in the project uses, and items only tests use (#1003): rustc's
# dead_code over a copy of the tree (under target/dead-pub/) with the lib's modules
# crate-private and every bin and the integration suite folded in. A signal, never a
# gate: it prints the report (and .lint/dead-pub.txt) and exits 0. About a minute
# warm; the first run checks the copied crate's dependencies once. Weekly on `main`
# in cargo-test.yml's `dead-pub` job, red only when no report came out (#1205).
dead-pub: ## Signal: pub items nothing uses, and items only tests use (tools/lint --dead-pub)
	$(LINT) -- --dead-pub

# The whole editor, headless, to a PNG (#731): what an editor-visible PR attaches.
# Needs a GPU or lavapipe. ARGS passes the binary's options (--select, --play, ...).
OUT ?= editor-capture.png
editor-capture: ## Render the editor to a PNG headlessly. make editor-capture OUT=after.png ARGS="--select Player"
	cargo run --quiet --features dev --bin editor-capture -- --out $(OUT) $(ARGS)

# Test games (#2029): build games/<NAME>/ into a project, headless, from its Lua recipe.
# Written to the worktree's target/games/<NAME>/; OUT=<dir> writes it elsewhere (e.g.
# to open it with the editor's --project). OUT has a default above (editor-capture's),
# so only one given on the command line or in the environment is passed on.
GAME_OUT = $(if $(filter command line environment,$(origin OUT)),--out $(OUT))
game: ## Generate a test game. make game NAME=smoke [OUT=<dir>] [ARGS="--param size=30"]
	cargo run --quiet --locked --features dev --bin game -- $(NAME) $(GAME_OUT) $(ARGS)

game-test: ## Generate a test game, check it is deterministic, run its scenarios and shots
	cargo run --quiet --locked --features dev --bin game -- $(NAME) $(GAME_OUT) --test $(ARGS)

# Rusty in the desktop's app search (#1315): the Linux app grid, Spotlight on macOS.
# Builds the dev editor (the window the `rusty` MCP attaches to) and session-mcp, then
# installs an entry in the user's home that starts this checkout's binary. Run it in
# the main checkout: a worktree is refused, since it goes away when its branch merges.
launcher: target-dir ## Put Rusty in the desktop's app search. ARGS=--remove takes it away
	@case "$(ARGS)" in *--remove*) ;; *) cargo build --quiet --features dev --bin rusty --bin session-mcp ;; esac
	@python3 tools/launcher/launcher.py $(ARGS)

# Ship a project (#1267): bring its converted sources (.fbx, .blend) up to date, then
# copy its runtime files and import cache into OUT, leaving those sources behind, so
# the player runs the result with no converter and no Blender. OUT must be new or
# empty, outside the project. Both are required: OUT has a default above.
export: ## Ship a project without its convertible sources. make export PROJECT=~/games/horde OUT=~/builds/horde
	@test -n "$(PROJECT)" && test "$(origin OUT)" = "command line" || { \
		echo "export: make export PROJECT=<project dir> OUT=<new or empty folder>" >&2; \
		exit 1; }
	cargo run --quiet --locked --features dev --bin export -- --project $(PROJECT) --out $(OUT)

# The shooter-shaped stress scene (#835): 50 skinned soldiers on the navmesh, 32
# lights, a full decal registry, smoke. Prints frame / render / GPU ms (avg and p95),
# GPU ms per pass, the counters and the heavy systems, with each row's change against
# the last run (kept in out/bench/bench.json). A signal, never a gate: the timings are
# this machine's. A release build, so the numbers are a shipped game's; the first run
# compiles one. docs/testing.md has the details.
bench: ## Benchmark the stress scene (release build), with the change against the last run
	cargo run --release --quiet --features dev --bin play -- --project tests/fixtures/project tests/fixtures/project/scenarios/bench/bench.lua out/bench
