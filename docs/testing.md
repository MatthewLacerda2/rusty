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

## Example (unit, in-module)
See `tools/lint/src/main.rs` — a `#[cfg(test)] mod tests` block testing the
size-cap selection and path normalization.

## Coverage
The `coverage` job in `ci.yml` measures what the suite exercises with
[`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov), accumulated over
**both feature sets** (default + `dev`) so it matches what CI actually runs. It
publishes two summaries to the run's job summary: the whole crate, and the
**deterministic sim modules** (`app`, `scripting`, `physics`, `navigation`).

Targets are deliberately differentiated:

- **~85% floor on the sim modules** — pure logic, no GPU, the part that must be
  right and where mutation/property testing is aimed.
- **No floor on the platform layer** (`main.rs`, `render`, `dev`) — headless
  coverage there is low-value.

The job is **informational** (`continue-on-error`) for now: it reports but does
not gate. The plan is to record a baseline and then **ratchet** — fail only on a
regression below it — rather than gate on a brittle absolute percentage. Run it
locally with `cargo llvm-cov --summary-only` (add `--features dev` for the
dev-only surface).

**When it runs:** post-merge on `main` (and on `workflow_dispatch`), **not** on
every pull request. The same applies to the report-only `mutants` sweep. Both
are non-blocking and take ~15 min, so on a PR they only ever sat "in progress"
long after the gating checks were green — a phantom stuck-PR. Running them on
`main` keeps the ratchet/baseline signal where it's actionable without stalling
PRs; a merge that touches no sim/code paths skips them via the `changes` filter.

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
