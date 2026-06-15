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
