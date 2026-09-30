# Testing conventions

| Kind | Lives in | Run with |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` in the same file | `cargo nextest run` |
| Integration tests | `tests/*.rs`, one binary rooted at `tests/main.rs` | `cargo nextest run` (or `cargo nextest run -E 'binary(integration)' <filter>`) |
| Doctests | `///` examples | `cargo test --doc` (nextest does not run them) |
| Harness scenarios | `project/scenarios/*.lua` (dev-only) | the headless `play` binary |

## Rules
- Test and fixture files are capped at **150 lines** by the size gate
  (`tools/lint`); keep them focused. Prefer small unit modules over one large
  test file.
- The sim must stay deterministic (fixed timestep, seeded RNG, no wall-clock in the
  tick) so harness/scenario tests are reproducible.
- CI (`.github/workflows/ci.yml`) and `make test` run the engine suite with
  **cargo-nextest** plus `cargo test --doc`, both feature sets; the lint xtask keeps
  plain `cargo test`. See *The test runner* below.

## The test runner: cargo-nextest
`make test` and CI run the suite with [cargo-nextest](https://nexte.st) (#484),
configured in `.config/nextest.toml`. Install it once: `cargo install --locked
cargo-nextest` (CI uses `taiki-e/install-action`). What it buys over `cargo test`:

- **Each test is its own process**, so one test's panic, leaked global or stuck
  thread cannot take its neighbours with it.
- **Every failure, summarised at the end** (`fail-fast = false`), instead of stopping
  at the first failing binary — the list an agent reads to decide what to fix.
- **A hang is named, not waited out**: a test is flagged SLOW after 60 s and killed
  after 5 min.
- **No retries.** A retried pass is still green, and a flaky test is a bug.

`cargo test` still works and is still correct — nothing in the suite depends on the
runner — but the gate is nextest. Doctests stay on `cargo test --doc`, which nextest
cannot run. CI's `ci` profile additionally writes JUnit, rendered as the
`build-test` job summary.

Handy forms: `cargo nextest run -E 'test(physics_)'` (filter by name),
`cargo nextest list` (what would run), `cargo nextest show-config test-groups`
(which tests the `gpu` group below holds).

## One integration-test binary
Cargo's default builds **each** `tests/*.rs` as its own executable, and each one links
the whole engine — wgpu/naga, egui, rapier3d, vendored Lua. With dozens of files and two
feature sets, that multiplied the slowest serial step of the build (linking) by the file
count and filled `target/` with near-identical binaries (#483). So `Cargo.toml` sets
`autotests = false` and declares a single `[[test]]` target, `integration`, rooted at
`tests/main.rs`; every file under `tests/` is a **module** of it.

- **A new test file needs a `mod` line** in `tests/main.rs` (or its folder's `mod.rs`) —
  Cargo no longer discovers it. `tests/layout.rs` fails the suite, naming the file, if
  one is forgotten, so a file can't silently never run.
- **Dev-only files are gated at the declaration**, `#[cfg(feature = "dev")] mod …;`,
  not by a `#![cfg]` inside the file.
- **Tests that render live in `tests/gpu/`**, so they share the `gpu::` module path and
  a runner can identify them by name (see the budget below).
- **Every module can share one process.** nextest gives each test its own, but
  `cargo test` runs the whole binary in one, and both must stay correct. Nothing in
  `tests/` may rely on having a process to itself: use a temp path unique to the file
  (not one another file also writes), and never `set_var` / `set_current_dir`.
- Filtering works by module path: `cargo nextest run -E 'binary(integration)' physics_`.

## GPU tests and the headless budget
Tests that need a real device call `Renderer::new_headless`, which returns `None`
when no adapter is present — so **every GPU test skips gracefully** rather than
failing. In-crate tests should acquire one through
`crate::render::test_gpu::headless_or_skip`, which carries that skip contract in one
place; the `let Some(r) = … else { return }` shape is the whole convention.

Where they actually run is not uniform, and it is worth knowing before you rely on one:

| CI job | Adapter | GPU tests |
|---|---|---|
| `build-test` (ubuntu) | Mesa **lavapipe** (software Vulkan, #489) | run, on the CPU |
| `build-test-cross` (macos) | real Metal GPU | run, against real VRAM |
| `build-test-cross` (windows) | **WARP** (software) | run, against **system RAM** |

**CI requires an adapter.** Those jobs set `RUSTY_REQUIRE_GPU=1`, which turns one
canary test (`render::test_gpu::tests::gpu_adapter_present_when_required`) from a skip into
a failure when no adapter is found — so a runner image that loses its driver goes red
instead of quietly skipping every GPU test. Without the variable (any local machine)
the skip contract above is unchanged.

**Running them on a GPU-less Linux box** (a container, a cloud session): install
lavapipe — `sudo apt-get install -y mesa-vulkan-drivers libvulkan1` — and the same
tests render there as in Linux CI. Set `RUSTY_REQUIRE_GPU=1` to be sure they did.

Two of the three CI adapters are software renderers, so a green run proves the render
path is correct on a conformant driver, not that it performs or behaves identically on
real hardware — Metal on macOS is the one real GPU. Pair anything load-bearing with an
adapter-free unit test on the underlying predicate so the rule is pinned everywhere.

**Concurrency is capped.** A `Renderer` is a device, the full pipeline set and shadow
maps, and Windows CI's WARP allocates all of that in system RAM shared with rustc — so
unbounded parallel renderers exhausted memory and failed *unrelated* tests with a bare
`Queue::write_texture: Not enough memory left.` (#366). Two caps, one per runner:

- **nextest (the gate):** every test is its own process, so nothing in-process can see
  its neighbours. The `gpu` **test group** in `.config/nextest.toml` (`max-threads = 2`)
  bounds how many GPU tests run at once.
- **`cargo test`:** `MAX_CONCURRENT_HEADLESS` in `src/render/setup/budget.rs` bounds
  how many renderers are alive in the one process, enforced by an RAII permit the
  renderer holds for its whole life. It applies to the normal library build, not just
  `cfg(test)`, because the screenshot integration tests reach the renderer indirectly
  through `screenshot::capture`. If a test hangs waiting on a permit, the guard panics
  with an explanation: something built a second headless renderer while still holding
  the first.

Keep the two numbers equal. If a new GPU test makes CI run out of memory, **lower both
to 1** before weakening the test.

**The GPU naming rule.** A test group selects tests by name, so the name says whether
a test renders:

- an **integration** test that renders lives under `tests/gpu/` (path `gpu::…`);
- an **in-crate** test that renders has a function name starting with **`gpu_`**
  (`fn gpu_resize_tracks_the_new_size`).

You cannot forget it quietly: in debug builds, a test that builds a headless renderer
without such a name panics and names this rule (`render::setup::gpu_rule`, called
from the budget's acquire). A test in that module also fails if the filter in
`.config/nextest.toml` stops matching the Rust predicate, so the two copies of the
rule cannot drift apart.

## API-doc drift gate
`tests/api_doc_drift.rs` (dev-only, #280) is a **hard gate** that keeps
`docs/scripting-api.md` honest against the **live Lua API surface**. It boots an
empty `Session`, walks the registered namespaces via
`ScriptManager::api_surface()` (every non-stdlib global table's function-valued
keys), and parses the doc's `##` namespace headings and table rows. It then asserts
**existence parity in both directions**: every documented `Namespace.Function` is a
registered binding, and every registered binding is documented (the failure message
lists the exact offenders per namespace). The doc is served to the agent over MCP
(#288), so a drifted doc would lie to it — this is what stops that. **Scope:**
existence only. Signatures/types are out of scope, because Lua closures are opaque at
runtime; checking them would need the deferred self-describing-binding macro. The
source of truth for existence is the live surface — reconcile by editing the doc.

The **callback half** of the surface has its own gate: `tests/callback_doc_drift.rs`
(#309, both feature sets) asserts the same existence parity between the doc's
"Script lifecycle callbacks" section and `src/scripting/callbacks.rs` — the one
list dispatch and MonoBehaviour discovery read — so a script callback can neither
be added undocumented nor advertised when the engine never dispatches it.

## Example (unit, in-module)
See `tools/lint/src/main.rs` — a `#[cfg(test)] mod tests` block testing the
size-cap selection and path normalization.

## Coverage
Coverage is measured with [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov),
accumulated over **both feature sets** (default + `dev`) so it matches what CI
actually runs. It is an **informational signal, never a merge blocker** (CLAUDE.md's
gates-vs-signals rule), surfaced where the agent acts on it.

Targets are deliberately differentiated:

- **A per-module floor on the sim modules** (`app`, `scripting`, `physics`,
  `navigation`) — pure logic, no GPU, the part that must be right and where
  mutation/property testing is aimed — **and on `api`** (#350), the Lua surface
  every game script drives: the doc drift gate proves a binding *exists*, only a
  behavioral test proves it *works* (`tests/<namespace>_api.rs`, one file per
  namespace). The floors live in `coverage-baseline.txt` at the repo root (one
  `module floor` line each) and **ratchet**: raise them as coverage improves,
  never lower them silently.
- **No floor on the platform layer** (`shell`, `render`, `dev`) — headless
  coverage there is low-value.

It runs in two tiers, both non-blocking (mirroring mutation testing, below):

| Run | Trigger | Scope | Where it lands |
|---|---|---|---|
| **PR run** (`coverage-pr`) | `pull_request`, `sim` filter | changed sim **lines** (`diff-cover`) | job summary **and** a sticky PR comment when changed lines are left uncovered |
| **Ratchet** (`coverage`) | post-merge on `main` + `workflow_dispatch` | per-sim-module total vs `coverage-baseline.txt` | a job-summary table flagging any module below its floor |

Why split it: the per-PR run scores only the lines the PR changed, so the agent
gets a *fresh-context catch* — did the new sim code get tested? — and fixes it
in-PR; the main run re-measures each whole module and ratchets it against the
committed floor as the backstop. Unlike mutation, diff-scoping does **not** make
the per-PR run cheap (the instrumented suite still runs in full), but it stays
informational and is not a required check, so it never gates a merge and may even
finish after one without stalling anything.

Both jobs measure under nextest (`cargo llvm-cov nextest`, #484), so the `gpu`
group caps them like `build-test`. Checked when it was adopted: on the default
feature set it produced the same totals as `cargo llvm-cov` to the line
(70.48% regions / 70.97% lines), from ~850 small per-process profiles (~280 MB).

Run it locally: `cargo llvm-cov nextest --summary-only` (add `--features dev` for the
dev-only surface); for the per-PR view, `cargo llvm-cov report --cobertura
--output-path cov.xml` then `diff-cover cov.xml --compare-branch origin/main`.

## Mutation testing
[`cargo-mutants`](https://github.com/sourcefrog/cargo-mutants) audits whether the
suite actually *catches* bugs — the headline guardrail against green-but-vacuous
agent-written tests. It mutates the deterministic sim (`app`, `scripting`,
`physics`, `navigation`), the pure-logic part where a silent bug hurts most, and
reports the **surviving** mutants (a change no test failed on). It runs in two
tiers, both **non-blocking** — mutation never gates a merge:

| Run | Trigger | Scope | Where it lands |
|---|---|---|---|
| **PR run** (`mutants-pr`) | `pull_request`, `sim` filter | `--in-diff` — only lines the PR changed (∩ the `--file` sim globs) | job summary **and** a sticky PR comment, so the coding agent fixes survivors in-PR |
| **Full sweep** (`mutants`, sharded) | nightly `schedule` + `workflow_dispatch`; skipped by `mutants-plan` when a finished sweep already covered `main`'s head (#506) — dispatch with `force_mutants` to re-run | full `--file` sim scope, split over a 10-shard matrix (`--shard k/10`) to stay under GitHub's 6 h job limit | `mutants-report` job: merged job summary (totals, unfinished shards, survivors) and the merged `mutants-report` artifact |

Why split it: diff-scoping makes the per-PR run fast and every survivor
attributable to a line the PR just wrote (the *fresh-context catch*), while the
nightly sweep re-examines untouched code the diff run never mutates and
tracks/ratchets the survivor backlog. Keeping both **informational** — surfaced
where the agent acts on them rather than failing the build — is deliberate:
`--in-diff` line-matching can drift after a rebase and timeouts can produce
spurious "survivors," neither of which should redden CI. The per-PR sticky
comment is cleared automatically once a re-push fixes the survivors.

**The mutation jobs stay on `cargo test`, not nextest** (#484). A mutant's cost is
the crate rebuild, not the test run, so nextest's scheduling buys little there — while
process-per-test adds a process spawn per test per mutant, and `fail-fast = false`
(right for the gate's summary) would make every *caught* mutant run the whole suite
instead of stopping at its first failing binary. The mutation jobs also have no GPU
driver, so the `gpu` group has nothing to bound. Revisit with
`cargo mutants --test-tool nextest` plus a dedicated fail-fast profile if mutant
test time ever dominates.

Run it locally (the diff-scoped form mirrors the PR run):
```
git diff origin/main > pr.diff
cargo mutants --in-diff pr.diff --file 'src/app/**/*.rs' -- --features dev   # ...plus the other sim modules
cargo mutants --no-shuffle --timeout-multiplier 3 -- --features dev          # full sweep
```

## Fuzzing (local-first)
The scene-load path is a parser eating untrusted input (hand-edited or corrupt
save files), so it gets a [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
target in `fuzz/`. The target (`scene_deserialize`) mirrors `load_from_file`:
arbitrary bytes → UTF-8 → `SceneData` (serde) → `apply_scene_data` (the
rehydration that rebuilds meshes/colliders/skeletons), surfacing panics, hangs,
and unguarded `unwrap`s.

libFuzzer is **nightly-only**, so `fuzz/` is its own workspace — deliberately
out of the pinned-`1.94.1` build, the size/determinism gates, and `cargo-deny`.
It is **local-first / on-demand**:

```
cargo +nightly fuzz run scene_deserialize          # fuzz until you stop it
cargo +nightly fuzz run scene_deserialize -- -max_total_time=60   # time-boxed
```

The committed `fuzz/corpus/scene_deserialize/` seeds the coverage-guided mutator
with the default scene plus a couple of minimal documents; new interesting inputs
accrete there. There is no CI gate — fuzzing is run for as long as the operator
wants; a short time-boxed CI smoke batch could be added later as a separate
nightly workflow if regression pressure is wanted.

## Parallel agent builds & disk

The agentic workflow runs issues concurrently in isolated git **worktrees**
(`.claude/worktrees/`), and each worktree gets its **own** Cargo `target/`. Since
#483 (one test binary) and #487 (the dev profile) that is ~2.5 GB after a dev build
and test compile and ~4.5 GB once `make gates` has built every feature set — it
used to be 12–16 GB. Disk still fails loudest, as `No space left on device` at the
**link** step (the tell-tale ENOSPC), but it is rarely what runs out now.

**Cores and memory are.** Memory now peaks mid-build, not at the final link,
while the optimised dependencies (wgpu/naga, rapier, image) compile side by side.
And one cold build already fills the cores: two side by side take as long as the
same two back to back, three as long as three. Running more at once buys no
throughput — it spreads the same CPU across more builds and spends the memory
headroom, about 3 GB per uncapped cold build. An incremental rebuild after an edit
takes seconds and about 1 GB; a fresh worktree with a warm compile cache (below)
builds in well under half the uncached time.

The policy below was set from measurements on the operator's 8-core / 15 GB Linux
machine on 2026-09-30 — #497 has the raw numbers. On a different machine,
re-measure rather than trust them.

Policy when driving parallel sub-agents:

- **At most two heavy builds at once** — a cold build of a fresh `target/` or a
  `make gates` run; incremental rebuilds don't count. Serialize the rest. (A PR that
  shares files with another in-flight PR must wait anyway — see the
  serialized-merge rule in `CLAUDE.md`.)
- **Half the cores each when two may overlap** — `CARGO_BUILD_JOBS=4` on 8 cores
  costs the pair nothing and saves ~2 GB; a build alone runs uncapped (capped, it
  takes ~25% longer).
- **Check headroom first** — before a heavy build, `free -g` should show ~5 GB
  available and `df -h` ~10 GB free; if not, wait for a sibling's build or clean
  finished worktree targets first.
- **Reclaim finished targets** — once a worktree's branch is pushed/merged,
  `rm -rf .claude/worktrees/<agent-dir>/target`.

This is an orchestration policy, not a hard gate: the disk ceiling belongs to
whichever machine the session runs on, so freeing space is the other lever. CI is
unaffected (each job runs on its own runner).

## The compile cache (sccache)

A worktree's own `target/` means every new worktree compiles all ~450 dependency
crates before it reaches a line of rusty. [sccache](https://github.com/mozilla/sccache)
removes most of that (#492). It stores each compile under a hash of its inputs —
source, flags, compiler, the outputs of its own dependencies — so a later worktree
gets the same artifact back. That is **not** a shared target dir: a hit is the same
output by construction, and nothing one worktree does can overwrite another's.

**Switching it on** is opt-in, once per machine: `cargo install --locked sccache`,
then `make setup` (or `make compile-cache` alone). It writes
`.claude/worktrees/.cargo/config.toml` in the main clone — gitignored, and read by
cargo in every worktree beneath it — naming sccache as the `rustc-wrapper` and
capping the cache at 2 GB (`make setup CACHE_SIZE=4G` to change it; sccache evicts
the oldest entries past the cap). Nothing else sees it: not the main checkout, not
the user's other projects, not CI, which keeps `Swatinem/rust-cache`. Without sccache
installed, `make setup` writes nothing. Delete the file to turn it off. The cache
lives in sccache's default directory (`~/.cache/sccache` on Linux,
`~/Library/Caches/Mozilla.sccache` on macOS) and holds a few hundred MB per
dependency set, compressed — count it when checking `df -h`.

What it does and does not cache, as measured when it was adopted (#492 has the
numbers):

- **Dependencies are cached**, and so is the C that build scripts compile through
  the `cc` crate (vendored Lua among it). A fresh worktree hit ~95% of compiles and
  its cold build took about a third of the uncached time. The misses are the crates
  whose build scripts generate code into `target/` (serde, thiserror, proc-macro2)
  and everything built on them (winit, egui-winit, gltf): that path is part of the
  hash, and every worktree's path is new.
- **rusty's own crate is never cached and stays incremental.** Cargo compiles
  workspace crates with `-C incremental`; sccache declines those and runs rustc
  untouched, so `target/debug/incremental` fills as before and a one-line edit
  rebuilds as fast as without the wrapper. This was #492's rejection condition: a
  cache that switched incremental compilation off would have slowed the edit loop,
  the loop that matters most.
- **Linked things are not cached**: build scripts, binaries and proc-macros. Linking
  rusty's own binaries is the floor under a cold build.
- **Switching the wrapper on or off does not invalidate `target/`**; cargo rebuilds
  nothing because of it.
