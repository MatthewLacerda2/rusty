# Linting & the commit gate

Programmatic, no AI in the loop. A commit is blocked unless the checks pass; the
result is written to `.lint/report.txt` so an agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -W clippy::too_many_lines -D warnings` (both feature sets) |
| Function ("endpoint") length | clippy `too_many_lines` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), enforced in CI; ~45 legacy functions grandfathered via `#[allow(clippy::too_many_lines)]` (burned down in #124) |
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

The function-length cap uses the same burn-down philosophy, but its "baseline" is the
set of `#[allow(clippy::too_many_lines)] // grandfathered: burn down in #124` markers
on the ~45 legacy functions that exceed 50 lines. New or changed code may not add such
an allow — split the function instead. #124 removes these markers one by one and then
flips the cap to enforce-for-all. Grep for the marker to see what's left:
```
grep -rn "too_many_lines" src/
```

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

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files). In **CI** it's a
hard gate — every warning fails the build, for both feature sets, including the
50-line function cap (`-W clippy::too_many_lines -D warnings`). The local pre-commit
hook (`lefthook.yml`) runs the same hard gate so offenders are caught before the
commit; CI re-runs it as the durable gate that survives `--no-verify`.
