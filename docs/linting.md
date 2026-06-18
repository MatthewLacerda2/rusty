# Linting & the commit gate

Programmatic, no AI in the loop. A commit is blocked unless the checks pass; the
result is written to `.lint/report.txt` so an agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (both feature sets) |
| Function ("endpoint") length | clippy `too_many_lines` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), enforced crate-wide via `-D clippy::too_many_lines`; no grandfathered functions remain |
| File length | `tools/lint` | <= 300 lines |
| Test / fixture file length | `tools/lint` | <= 150 lines |
| Sim determinism | `tools/lint -- --determinism` | no `Instant::now`/`SystemTime`/`rand::random` in `app`/`scripting`/`physics`/`navigation` |
| Component completeness | `tools/lint -- --components` | every first-class component has all 4 axes (field, Add Component entry, inspector card, API namespace), minus the baseline |

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
A first-class component is only "done" when it appears on all four axes that
deliberately live in non-dependent layers: a field on `Entity`, an Add Component
entry (`inspector_add.rs`), an inspector card (some `inspector_*.rs`), and an API
namespace (`src/api/<x>.rs` registered in `api/mod.rs` and documented in
`scripting-api.md`). The gate discovers components from `Entity`'s
`Option<…Component>` fields — so a new one can't slip through — and fails on any
missing axis. `tools/lint/components_baseline.txt` grandfathers incomplete
components as `<component> <axis>` lines (the same burn-down rule as above). As of
#82 that file is **empty** — every grandfathered gap was either closed or waived.

### Closed vs waived (#82)
An axis can be satisfied in two ways:

- **Closed** — the artifact exists (the field, Add Component entry, inspector card,
  or `src/api/<x>.rs` namespace + doc). #82 closed `animator add_menu` (Animator got
  its own Add Component entry, decoupled from Health) and `light api` (the new
  `src/api/light.rs` `Light` namespace).
- **Waived** — a documented decision *not* to add a per-component artifact, because
  the axis is already served by a shared namespace or a content-driven workflow, and
  doing it standalone would fragment the one stable API surface. Waivers live in the
  `WAIVERS` table in `tools/lint/src/components.rs` as `(component, axis, rationale)`
  rows: `mesh add_menu`/`mesh api` (mesh is content-grid/glTF-driven),
  `texture api` (→ `Material`), `collider api` + `rigidbody api` (→ `Physics`),
  `nav_agent api` (→ `NavMeshAgent`), `visual_correction api` (→ `Graphics`).

The difference is intent: the **baseline** is a burn-down list of axes we still mean
to implement; **`WAIVERS`** is for axes we have decided not to implement standalone.
A waiver is never a silent skip — its rationale lives in code and is reviewable in
`git`. To add one, add a row to `WAIVERS` with a clear justification; to revisit one,
delete the row and the gate will demand the artifact again. The particle system is on
neither list — it is the gate's first fully-green component, satisfied on all axes.

## The function-length cap (`too_many_lines`)
The 50-line per-function cap is a **hard clippy gate** (`-D clippy::too_many_lines`,
both feature sets) and is now **unconditionally on for the whole crate** — the legacy
functions that predated it have all been split (#124), so **no
`#[allow(clippy::too_many_lines)]` remain anywhere in `src/`**. Verify with:
```
rg -c 'allow\(clippy::too_many_lines\)' src   # expect no output
```
Keep it that way: if a function grows past 50 lines, split it — **do not** silence the
lint with a new `#[allow]`.

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files). In **CI** it's a
hard gate — every warning (plus the function-length cap) fails the build, for both
feature sets. The local pre-commit hook (`lefthook.yml`) runs the same hard clippy
gate on the default feature set; CI additionally covers the dev feature set and is
the durable gate that gates the PR.
