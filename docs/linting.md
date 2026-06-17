# Linting & the commit gate

Programmatic, no AI in the loop. A commit is blocked unless the checks pass; the
result is written to `.lint/report.txt` so an agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (both feature sets) |
| Function ("endpoint") length | clippy `too_many_lines` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), enforced via `-D clippy::too_many_lines`; legacy functions grandfathered with inline `#[allow]` |
| File length | `tools/lint` | <= 300 lines |
| Test / fixture file length | `tools/lint` | <= 150 lines |
| Sim determinism | `tools/lint -- --determinism` | no `Instant::now`/`SystemTime`/`rand::random` in `app`/`scripting`/`physics`/`navigation` |
| Component completeness | `tools/lint -- --components` | every built-in component has all 4 axes (field, Add Component entry, inspector card, API namespace), minus the baseline |

## Run it
```
cargo run --manifest-path tools/lint/Cargo.toml          # size gate, full scan
cargo fmt --check                                        # style
cargo clippy --no-deps                                   # smells
```

## Where it runs
- **Local:** `lefthook.yml` (install once with `lefthook install`).
- **CI:** `.github/workflows/lint.yml` — the durable layer; survives `--no-verify`.

## The baseline (burn-down list)
`tools/lint/baseline.txt` grandfathers the files that already exceed the cap. It is a
**TODO list, not a pardon**: as a file is split, remove its entry. Never add new
entries. When the file is empty, the size gate is fully on.

## Component completeness (`--components`)
A built-in component is only "done" when it appears on all four axes that
deliberately live in non-dependent layers: a field on `Entity`, an Add Component
entry (`inspector_add.rs`), an inspector card (some `inspector_*.rs`), and an API
namespace (`src/api/<x>.rs` registered in `api/mod.rs` and documented in
`scripting-api.md`). The gate discovers components from `Entity`'s
`Option<…Component>` fields — so a new one can't slip through — and fails on any
missing axis. `tools/lint/components_baseline.txt` grandfathers today's incomplete
components as `<component> <axis>` lines (the same burn-down rule as above; #82
removes them one axis at a time). The particle system is intentionally absent from
that baseline — it is the gate's first fully-green component.

## The function-length cap (`too_many_lines`)
The 50-line per-function cap is a **hard clippy gate** (`-D clippy::too_many_lines`,
both feature sets). The functions that predate the cap are grandfathered with an
inline `#[allow(clippy::too_many_lines)]` on each one. That attribute **is** the
burn-down list — count it with:
```
rg -c 'allow\(clippy::too_many_lines\)' src
```
Same rule as the size baseline: as you split a grandfathered function under 50 lines,
delete its `#[allow]`; **never add a new one**. When none remain, the cap is
unconditionally on for the whole crate.

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files). In **CI** it's a
hard gate — every warning (plus the function-length cap) fails the build, for both
feature sets. The local pre-commit hook (`lefthook.yml`) runs the same hard clippy
gate on the default feature set; CI additionally covers the dev feature set and is
the durable gate that gates the PR.
