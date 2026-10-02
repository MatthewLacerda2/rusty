# Linting & the commit gate

Programmatic, no AI in the loop. `make gates` runs every check below; the commit
hook runs the fast ones. `tools/lint` writes its result to `.lint/report.txt` so an
agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (default, `--features dev`, and `--no-default-features` — the editor-free player build) |
| Merge helpers | `unittest` (`make scripts`) | **hard gate**: `.github/scripts/tests` pass — `make mergeable` is the only thing between a red PR and `main` while it has no required checks (#486, #491) |
| Function ("endpoint") length | clippy `too_many_lines` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), denied crate-wide in `Cargo.toml`'s `[lints]` |
| File length | `tools/lint` | <= 300 lines |
| Test / fixture file length | `tools/lint` | <= 150 lines (standalone `*_test.rs` / `tests/` / `fixtures/`); a `<x>_tests.rs` **sibling** of `<x>.rs` shares the 300-line source cap |
| Sim determinism | `tools/lint -- --determinism` | no `Instant::now`/`SystemTime`/`rand::random` in `app`/`scripting`/`physics`/`navigation`/`ui`/`api`/`scene`/`components`/`ecs`/`core`/`time`/`asset`/`procgen`/`shadergen`/`audio` |
| Dependency direction | `tools/lint -- --direction` | sim modules (the determinism list) never reference `crate::render`, `crate::editor`, `wgpu` or `egui` — the arrow is render/editor → sim (#494) |
| Sim panic-freedom | clippy `unwrap_used` | **hard gate**: `#![deny(clippy::unwrap_used)]` in `app`/`scripting`/`physics`/`navigation`/`ui`; bare `.unwrap()` banned in production (test code exempt via `allow-unwrap-in-tests`) |
| Component completeness | `tools/lint -- --components` | every first-class component has all 4 axes (field, Add Component entry, inspector card, API namespace), minus the baseline |
| Editor↔shared-op parity | `tools/lint -- --parity` | every *migrated* first-class component's inspector card routes its mutations through a shared `scene::authoring` op (never direct field writes through the #344 accessor guard), minus the burn-down baseline |
| Rust API reference | rustdoc | **hard gate**: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` (both feature sets) — no broken intra-doc links, no public docs linking private items; name a private item as plain `code`, not a link |

## The `*_tests.rs` sibling rule
The tight test cap exists to discourage over-splitting a bundle of `#[test]`s. But a
`<x>_tests.rs` file sitting next to its `<x>.rs` source is the *opposite* — it is that
one source's dedicated, single test home. Forcing it under the 150-line cap pushes the
overflow back into an inline `#[cfg(test)] mod tests`, so the same source ends up tested
from *two* places. To keep cohesion, a `<x>_tests.rs` **sibling** shares its source's
300-line cap (source + sibling read as one logical unit); the standalone `*_test.rs`
(singular), `tests/`, and `fixtures/` forms keep the tight 150-line cap (#211).

## What the size gate scans
The full scan (`make size`, CI's `lint` job) walks `src/`, `tests/`, `fuzz/` and
`tools/` (`SCAN_ROOTS` in `tools/lint/src/size.rs`); until #535 it walked only `src/`,
so oversized integration-test files passed unseen, and until #541 it skipped `tools/`,
so the lint crate itself could drift past the cap. Any `target/` directory (cargo
build output, e.g. `tools/lint/target/` or `fuzz/target/`) is never descended into.
The commit hook checks whatever `.rs` files are staged, wherever they live.

## Run it
```
make gates     # everything CI blocks on, fastest-failing first
make check     # the fast edit loop: formatting, size gate, dev-feature clippy
make help      # every verb, and each gate on its own (make size, make clippy, …)
```
`make gates` starts with two self-checks. **`target-dir`** asks cargo where it
builds and refuses to run when that is outside the worktree: a target dir shared
between worktrees lets one branch's artifacts stand in for another's, so a green
run would not prove *this* branch compiled. **`inventory`** fails when a Makefile
target documented `## [gate]` is missing from the `GATES` list, or the other way
round, so the gate list cannot silently lose a gate.

## Where it runs
- **Commit hook:** `.githooks/pre-commit` → `make pre-commit`: `cargo fmt --check`
  and the size gate on the staged `.rs` files — nothing else, so it stays under a
  second. Activate it once per clone with `make setup` (cloud sessions do it in
  `.claude/hooks/session-start.sh`). `git commit --no-verify` skips it; CI doesn't.
  `make setup` also switches the worktrees' compile cache on when sccache is
  installed (`docs/testing.md`, *The compile cache*).
- **Before readying a PR:** `make gates`.
- **CI:** `.github/workflows/{ci,lint}.yml` — the durable layer; survives `--no-verify`.

## The lint policy lives in `Cargo.toml`
Crate-wide lint levels go in `Cargo.toml`'s `[lints]` table, never in command-line
flags: a plain `cargo clippy`, rust-analyzer and CI then all read the same policy,
so nothing is green locally and red in CI. CI adds only `-D warnings`. Thresholds
stay in `clippy.toml`. Module-scoped rules stay module-scoped — the sim modules'
`#![deny(clippy::unwrap_used)]` below is deliberately not crate-wide. `tools/lint`
has no `[lints]` table: it is not clippy-gated, so one would be dead config.

## The baseline (burn-down list)
`tools/lint/baseline.txt` grandfathers the files that already exceed the cap. It is a
**TODO list, not a pardon**: as a file is split, remove its entry. Never add new
entries. When the file is empty, the size gate is fully on.

## Dependency direction (`--direction`)
The sim runs headless with no GPU and no UI, so the dependency arrow points one way:
`render` and `editor` import sim types, never the reverse. `tools/lint -- --direction`
scans the sim modules (`app`, `scripting`, `physics`, `navigation`, `ui`, `api`,
`scene`, `components`, `ecs`, `core`, `time`, `asset`, `procgen`, `audio`) and fails on any non-comment reference
to `crate::render`, `crate::editor`, `wgpu` or `egui` (whole path segments only).
Plain data the renderer and the sim share lives sim-side: the mesh `Vertex` and the
primitive builders in `components::mesh`, `Camera` and `Decal` in `scene`,
`QualityPreset` in `core::quality`; the renderer keeps only the GPU half (e.g.
`render::gpu::mesh::vertex_layout`). There is no baseline — the guard landed with
zero violations (#494), and it is the groundwork the crate split (#495) needs.
`api` joined the scan in #723: it is the surface scripts drive from inside the sim.
`shadergen` joined in #722, once its GPU-free `naga_oil` composition moved out of
`render` into `shadergen::compose` (the renderer's `ShaderRegistry` now calls it).
Both guards fail on
a listed directory that no longer exists, and `test-lint` pins each list, so a
module can only leave a scan through a reviewed edit.

## Component completeness (`--components`)
A first-class component is only "done" when it appears on all four axes that
deliberately live in non-dependent layers: a field on `Entity`, an Add Component
entry (`inspector/components/add/`), an inspector card (some `inspector/components/*.rs`), and an API
namespace (`src/api/<x>.rs` registered in `api/mod.rs` and documented in
`docs/api/`). The gate discovers components from `Entity`'s
`Option<…Component>` fields — so a new one can't slip through — and fails on any
missing axis. `tools/lint/components_baseline.txt` grandfathers incomplete
components as `<component> <axis>` lines (the same burn-down rule as above). As of
#82 that file is **empty** — every grandfathered gap was either closed or waived.

### Closed vs waived (#82)
An axis can be satisfied in two ways:

- **Closed** — the artifact exists (the field, Add Component entry, inspector card,
  or `src/api/<x>.rs` namespace + doc). #82 closed `animator add_menu` (Animator got
  its own Add Component entry, added/removed independently) and `light api` (the new
  `src/api/light.rs` `Light` namespace).
- **Waived** — a documented decision *not* to add a per-component artifact, because
  the axis is already served by a shared namespace or a content-driven workflow, and
  doing it standalone would fragment the one stable API surface. Waivers live in the
  `WAIVERS` table in `tools/lint/src/components/waivers.rs` as `(component, axis, rationale)`
  rows: `mesh add_menu`/`mesh api` (mesh is content-grid/glTF-driven),
  `texture api` (→ `Material`), `collider api` + `rigidbody api` (→ `Physics`),
  `nav_agent api` (→ `NavMeshAgent`), `visual_correction api` (→ `Graphics`).

The difference is intent: the **baseline** is a burn-down list of axes we still mean
to implement; **`WAIVERS`** is for axes we have decided not to implement standalone.
A waiver is never a silent skip — its rationale lives in code and is reviewable in
`git`. To add one, add a row to `WAIVERS` with a clear justification; to revisit one,
delete the row and the gate will demand the artifact again. The particle system is on
neither list — it is the gate's first fully-green component, satisfied on all axes.

## Editor↔shared-op parity (`--parity`)
A first-class component is authored from two places that must never drift: the
editor's inspector card and the Lua API namespace. The fix is to make them siblings
over **one shared `scene::authoring` op** — the card AND the Lua binding both call it,
so the field write and its validation live once and can't diverge. The `material`
card is the first migrated capability (#287): its widgets read fields immutably and
route every change through `scene::authoring::material::*`, the same ops the
`Material.*` Lua API calls.

This gate keeps migrated cards honest. It discovers components the same way
`--components` does — from `Entity`'s `Option<…Component>` fields — and, for each one
that has an inspector card, classifies the card as:

- **routed** — reads a snapshot immutably and routes every write through an
  `authoring::…` op: the component's mutable accessor (`world.<field>_mut(id)`,
  #344) is only ever taken to hand `&mut c` into a shared op. Detaching on remove
  (`world.set_<field>(id, None)`) is *not* a field mutation, so a routed card may
  still do it; a read-only card (e.g. `mesh`) trivially qualifies.
- **direct** — takes the component's mutable accessor with no `scene::authoring`
  ops for that component in scope, and writes fields through egui widgets — the
  regression shape this gate exists to stop.

The detection signal is facade-shaped (#344): `.<field>_mut(` present in the cards
with no `authoring::<field>` ops import marks a direct card. Coarse substring scan,
matching the other gates.

### Routed vs direct, and the burn-down (#287)
`tools/lint/parity_baseline.txt` grandfathers the not-yet-migrated cards as bare
component names (the same burn-down rule as the other baselines). Enforcement:

- A **direct** card *not* in the baseline → **violation** (drift: a direct mutation
  was added without grandfathering it).
- A component in the baseline whose card is actually **routed** → **violation** (stale
  entry — you migrated the card but left it listed; remove the line). This forces
  burn-down hygiene, exactly like the size/components baselines.
- A **routed** card *not* in the baseline → ok. This is the gate's **teeth**:
  `material` is absent and must pass as routed, so reverting it to a direct mutation
  fails the build.
- A **direct** card *in* the baseline → ok (grandfathered, awaiting migration).

This is a **hard gate** (CI `--parity` step, both in the lint workflow): correctness
of the no-drift invariant, green-to-merge. Burn it down by migrating a card to a
shared op and removing its baseline line — never add a new line (a new direct card is
fresh drift to fix, not to grandfather; baseline only, exceptionally, with written
justification).

## The function-length cap (`too_many_lines`)
The 50-line per-function cap is a **hard clippy gate** (`too_many_lines = "deny"` in
`Cargo.toml`'s `[lints.clippy]`, both feature sets) and is **on for the whole crate**
— the legacy functions that predated it were split (#124). The one remaining
`#[allow(clippy::too_many_lines)]` is `Renderer::assemble` in
`src/render/setup/mod.rs`, a flat one-line-per-field constructor whose length is the
struct's width. List any with:
```
rg 'allow\(clippy::too_many_lines\)' src
```
If a function grows past 50 lines, split it — **do not** silence the lint with a
new `#[allow]`.

The core sim modules (`app`, `scripting`, `physics`, `navigation`, `ui`)
carry a module-level `#![deny(clippy::unwrap_used)]`, so a bare `.unwrap()` in
their **production** code is a hard clippy error (caught by the same `-D warnings`
gate, both feature sets, `make gates` + CI). This is the **survivability**
sibling of the determinism guard's **reproducibility**: a `.unwrap()` that panics
mid-frame kills an unattended/overnight play-test just as surely as a wall-clock
read breaks replay — same rationale (#195). The determinism guard has since widened
past these five (#723); extending the unwrap ban to the wider set is a separate
burn-down, not yet measured.

The rule is deliberately narrow:
- **`unwrap`, not `expect`.** `.expect("clear invariant")` is the sanctioned escape
  hatch — it documents *why* the value must be present at the call site. Use `?`
  where a `Result` should propagate, `.expect(...)` where the invariant is real and
  local; reach for the bare `.unwrap()` nowhere in the sim core.
- **Sim modules only.** The platform layer (`shell`, `render`, `dev`) is exempt —
  e.g. `render/gpu/shaders.rs` panicking at boot on a bad shader is fail-fast-at-startup,
  not a mid-sim hazard. The determinism guard's exemption is the same platform layer.
- **Tests exempt.** `clippy.toml`'s `allow-unwrap-in-tests = true` lets test code
  unwrap freely, so no per-test `#[allow]` noise.

There is no baseline/burn-down: the sim core landed already clean (its few unwraps
were all in `#[cfg(test)]`), so the lint went straight to `deny` with no grandfathered
sites. Keep it that way — fix the call site, don't add an `#[allow]`.

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files), which is too
slow for a commit hook — so the hook doesn't run it. `make gates` and CI run the
same hard clippy gate (`-D warnings`, both feature sets); CI is the durable one that
gates the PR.
