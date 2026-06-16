# Linting & the commit gate

Programmatic, no AI in the loop. A commit is blocked unless the checks pass; the
result is written to `.lint/report.txt` so an agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (both feature sets) |
| Function ("endpoint") length | clippy `too_many_lines` | `too-many-lines-threshold = 50` — configured in `clippy.toml`, not wired into CI |
| File length | `tools/lint` | <= 300 lines |
| Test / fixture file length | `tools/lint` | <= 150 lines |
| Sim determinism | `tools/lint -- --determinism` | no `Instant::now`/`SystemTime`/`rand::random` in `app`/`scripting`/`physics`/`navigation` |
| Component completeness | `tools/lint -- --components` | every built-in component is wired across all its axes (entity field, Add Component entry, inspector card, API namespace, docs) |

## Run it
```
cargo run --manifest-path tools/lint/Cargo.toml               # size gate, full scan
cargo run --manifest-path tools/lint/Cargo.toml -- --determinism   # sim-purity guard
cargo run --manifest-path tools/lint/Cargo.toml -- --components    # component completeness
cargo fmt --check                                             # style
cargo clippy --no-deps                                        # smells
```

## Where it runs
- **Local:** `lefthook.yml` (install once with `lefthook install`).
- **CI:** `.github/workflows/lint.yml` — the durable layer; survives `--no-verify`.

## Component completeness (`--components`)
A built-in component is only "done" when it is wired across the layers that
deliberately don't depend on each other — so the check can't be a compile-time enum.
The gate **discovers** every `…Component` struct under `src/components/` (minus the
mandatory `Transform`) and asserts each one is present on all of its axes:

| Axis | Where it's checked |
|---|---|
| **field** | an `Option<…>` field on `Entity` (`src/components/entity.rs`) |
| **add_menu** | a dedicated `entity.<field>.is_none(` Add Component entry (`src/editor/inspector_add.rs`) |
| **inspector** | an inspector card — `fn draw_<field>` or a dedicated `inspector_<field>.rs` |
| **api** | a registered API namespace serving the component (`src/api/`, registered in `src/api/mod.rs`) |
| **docs** | that namespace documented in `docs/scripting-api.md` |

A new component that's missing any axis **fails the build**. The particle system is
the gate's first fully-green component; today's incomplete ones are grandfathered in
`tools/lint/components_baseline.txt` (format: `<field> <axis>`) and burned down in
issue #82. The baseline only shrinks — a line whose axis is later satisfied is
flagged `STALE_BASELINE` and must be deleted.

## The baseline (burn-down list)
`tools/lint/baseline.txt` grandfathers the files that already exceed the cap. It is a
**TODO list, not a pardon**: as a file is split, remove its entry. Never add new
entries. When the file is empty, the size gate is fully on. The component gate has its
own burn-down list (`components_baseline.txt`, above) with the same discipline.

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files). In **CI** it's a
hard gate — every warning fails the build, for both feature sets. The local
pre-commit hook runs clippy **report-only** (it writes `.lint/clippy.txt` and never
blocks the commit) so a commit isn't held up by an unrelated crate-wide warning; CI
is the durable gate that gates the PR.
