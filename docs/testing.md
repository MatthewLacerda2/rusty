# Testing conventions

| Kind | Lives in | Run with |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` in the same file | `cargo test` |
| Integration tests | `tests/*.rs` (crate-level) | `cargo test` |
| Harness scenarios | `project/scenarios/*.lua` (dev-only) | the headless `play` binary |

## Rules
- Test and fixture files are capped at **150 lines** by the size gate
  (`tools/lint`); keep them focused. Prefer small unit modules over one large
  test file.
- The sim must stay deterministic (fixed timestep, seeded RNG, no wall-clock in the
  tick) so harness/scenario tests are reproducible.
- CI (`.github/workflows/ci.yml`) runs `cargo test` for both the engine and the
  lint xtask.

## GPU tests and the headless budget
Tests that need a real device call `Renderer::new_headless`, which returns `None`
when no adapter is present — so **every GPU test skips gracefully** rather than
failing. In-crate tests should acquire one through
`crate::render::test_gpu::headless_or_skip`, which carries that skip contract in one
place; the `let Some(r) = … else { return }` shape is the whole convention.

Where they actually run is not uniform, and it is worth knowing before you rely on one:

| CI job | Adapter | GPU tests |
|---|---|---|
| `build-test` (ubuntu) | none installed | **skip** — the `None` path, ~0.00s |
| `build-test-cross` (macos) | real Metal GPU | run, against real VRAM |
| `build-test-cross` (windows) | **WARP** (software) | run, against **system RAM** |

Because Linux skips them, a GPU test passing locally in a container proves nothing;
macOS and Windows CI are where it is really exercised. Pair anything load-bearing with
an adapter-free unit test on the underlying predicate so the rule is pinned everywhere.

**Concurrency is capped.** A `Renderer` is a device, the full pipeline set and shadow
maps, and Windows CI's WARP allocates all of that in system RAM shared with rustc — so
unbounded parallel renderers exhausted memory and failed *unrelated* tests with a bare
`Queue::write_texture: Not enough memory left.` (#366). `MAX_CONCURRENT_HEADLESS` in
`src/render/setup/budget.rs` now bounds how many are alive at once, enforced by an RAII
permit the renderer holds for its whole life. It applies to the normal library build,
not just `cfg(test)`, because the screenshot integration tests reach the renderer
indirectly through `screenshot::capture`.

You do not need to do anything to opt in — but if a new GPU test makes CI run out of
memory, **lower that constant to 1** before weakening the test. If a test hangs waiting
on a permit, the guard panics with an explanation: it means something built a second
headless renderer while still holding the first.

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
- **No floor on the platform layer** (`main.rs`, `render`, `dev`) — headless
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

Run it locally: `cargo llvm-cov --summary-only` (add `--features dev` for the
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
| **Full sweep** (`mutants`) | nightly `schedule` + `workflow_dispatch` | full `--file` sim scope | uploaded survivor artifact (`mutants-report`) |

Why split it: diff-scoping makes the per-PR run fast and every survivor
attributable to a line the PR just wrote (the *fresh-context catch*), while the
nightly sweep re-examines untouched code the diff run never mutates and
tracks/ratchets the survivor backlog. Keeping both **informational** — surfaced
where the agent acts on them rather than failing the build — is deliberate:
`--in-diff` line-matching can drift after a rebase and timeouts can produce
spurious "survivors," neither of which should redden CI. The per-PR sticky
comment is cleared automatically once a re-push fixes the survivors.

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
(`.claude/worktrees/`), and each worktree gets its **own** Cargo `target/` —
~12–16 GB once built. The session volume is ~38 GB usable, so fanning out N
parallel builds needs N × ~15 GB and hits `No space left on device` — failing at
the **link** step, not compile, which is the tell-tale ENOSPC.

Policy when driving parallel sub-agents:

- **Cap concurrent heavy builds** — at most ~2 worktree builds at once on a
  ~38 GB volume; serialize the rest. (A PR that shares files with another
  in-flight PR must wait anyway — see the serialized-merge rule in `CLAUDE.md`.)
- **Reclaim finished targets** — once a worktree's branch is pushed/merged,
  `rm -rf .claude/worktrees/<agent-dir>/target` frees ~12–16 GB.
- **Check headroom first** — if `df -h` shows < ~16 GB free before launching a
  build, clean finished worktree targets first.

This is an orchestration policy, not a hard gate: the disk ceiling is fixed at
environment creation, so raising the session volume is the other lever. CI is
unaffected (each job runs on its own runner).
