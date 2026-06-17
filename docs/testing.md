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
