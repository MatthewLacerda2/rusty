# Linting & the commit gate

Programmatic, no AI in the loop. A commit is blocked unless the checks pass; the
result is written to `.lint/report.txt` so an agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (both feature sets) |
| Function ("endpoint") length | clippy `too_many_lines` | `too-many-lines-threshold = 50` — configured but not yet gated (legacy burn-down) |
| File length | `tools/lint` | <= 300 lines |
| Test / fixture file length | `tools/lint` | <= 150 lines |
| Sim determinism | `tools/lint -- --determinism` | no `Instant::now`/`SystemTime`/`rand::random` in `app`/`scripting`/`physics`/`navigation` |

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
`tools/lint/baseline.txt` grandfathers the legacy monoliths that already exceed the
cap. It is a **TODO list, not a pardon**: as a file is split or migrated, remove its
entry. Never add new entries. When the file is empty, the size gate is fully on.

## Clippy phasing
The legacy modules aren't clippy-clean yet, and clippy lints a whole crate (it can't
be scoped to changed files). So clippy is **report-only** until the migration lands,
then it becomes a hard gate.
