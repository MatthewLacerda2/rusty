# Linting & the commit gate

Programmatic, no AI in the loop. `make gates` runs every check below; the commit
hook runs the fast ones. `tools/lint` writes its result to `.lint/report.txt` so an
agent can read exactly what failed.

## What's enforced
| Check | Tool | Rule |
|---|---|---|
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (default, `--features dev`, and `--no-default-features` — the editor-free player build), plus `tools/lint` itself (#935) |
| Merge helpers | `unittest` (`make scripts`) | **hard gate**: `.github/scripts/tests` pass — `make mergeable` is the only thing between a red PR and `main` while it has no required checks (#486, #491) |
| Function ("endpoint") length | clippy `too_many_lines` + `tools/lint -- --determinism` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), denied crate-wide in `Cargo.toml`'s `[lints]`; its one exception, `Renderer::assemble`, is locked (`ESCAPE`, #961) |
| Lint exceptions carry a reason | clippy `allow_attributes` + `allow_attributes_without_reason` | **hard gate**: an exception is `#[expect(lint, reason = "…")]` (outer `#[allow]` is denied; a module-root `#![allow]` still needs its `reason`), and an expectation that no longer fires fails too (#961) |
| File length | `tools/lint` | <= 300 lines: every line of a `.rs` file, the lines of code of a `.py`, `.lua` or `.wgsl` one (docstrings, comments and blank lines free, #926, #1015); nothing grandfathered |
| Test / fixture file length | `tools/lint` | <= 150 lines (standalone `*_test.rs` / `tests/` / `fixtures/`, a `*_tests.rs` with no source beside it, and every `.py` under a `tests/` directory); a `<x>_tests.rs` **sibling** of `<x>.rs` (or of a `<x>/` module directory) shares the 300-line source cap |
| Folder grouping | `tools/lint -- --rules` | **hard gate** (`prefix-group`): no three sibling `.rs` files share a first word (`nav_agent.rs`, `nav_obstacle.rs`, …); make it a subfolder and drop the prefix. Prints the ≲10-files-per-directory signal beside it, never failing on it (#934) |
| Nothing temporary | clippy `dbg_macro`/`todo`/`unimplemented` + `tools/lint -- --rules` (`markers`) | **hard gate**: the three are denied in `Cargo.toml`'s `[lints]` (and `tools/lint`'s); a marker word (`TODO`, `FIXME`, `XXX`, `HACK`) in a scanned `.rs` file fails — an unfinished thing is an issue (#935) |
| Test source shape | `tools/lint -- --rules` (`test-inner-cfg`, `test-user-workspace`, `test-gpu-entry`) | **hard gate**: no inner `#![cfg]` in `tests/`, no user workspace in tests, a GPU only through `headless_or_skip` (#980) |
| Full component literals in tests | `tools/lint -- --rules` (`full-literal`) | **hard gate**: test code builds a first-class component with `..rest` or a constructor, never a literal naming every field (#663, #976) |
| Skipped tests | `tools/lint -- --rules` (`skip`) | **hard gate**: `#[ignore]` / `#[cfg_attr(…, ignore)]` needs `// lint: allow(skip) <why>` (#935) |
| Pushed = built | `tools/lint -- --rules` (`ignored`) | **hard gate**: no scanned `.rs` file may match a `.gitignore` pattern, tracked or not; no allow (#935) |
| No subprocesses | `tools/lint -- --rules` (`subprocess`) | **hard gate**: no line of `src/` names `std::process::Command` — the engine never shells out (to Blender or anything else); a real need takes `// lint: allow(subprocess) <why>` (#975) |
| Dependency locks | cargo-deny (`deny.toml`) | **hard gate**: every git dependency is pinned by `rev` (`required-git-spec`); `nalgebra` only through the physics stack and no `cgmath` — glam is the one math crate (#975) |
| `tools/lint` stays std-only | `make test-lint` | its `Cargo.lock` names one package, itself (#935) |
| Test temp dirs | `tools/lint -- --rules` | **hard gate** (`test-temp-dir`): test code writes under `crate::test_temp::dir()` / `crate::temp::dir()`, never a bare `std::env::temp_dir()` (#999) |
| Module layering | `tools/lint -- --layers` | every `crate::<module>` import and every external crate a module names matches the declared table (`tools/lint/src/layers/table.rs`): nothing undeclared, nothing stale, no cycle, no sim → platform edge (#724, #938) |
| Sim determinism | clippy `disallowed_methods`/`disallowed_types` + `tools/lint -- --determinism` | **hard gate**: clock reads, OS/unseeded RNG and `std`'s randomly seeded hash maps are banned crate-wide in `clippy.toml`; only a platform row's `mod.rs` may opt out, and the ban list must stay whole (#757, #764) |
| No shared process state | clippy `disallowed_methods` + `tools/lint -- --determinism` | **hard gate**: `std::env::{set_var, remove_var, set_current_dir}` are banned crate-wide in `clippy.toml`, tests included, since `cargo test` runs every in-crate test in one process; production opts out inline with a reason (`project::open`), and `--determinism` keeps the three on the list (#977) |
| Dependency direction | `tools/lint -- --direction` | the table's sim modules never reference `crate::render`, `crate::editor`, `wgpu` or `egui` — the arrow is render/editor → sim (#494) |
| Sim panic-freedom | clippy `unwrap_used` + `tools/lint -- --rules` (`unwrap-deny`) | **hard gate**: every sim row of the layer table carries `#![deny(clippy::unwrap_used)]` at its `mod.rs`, and no sim file lifts it; bare `.unwrap()` banned in production (test code exempt via `allow-unwrap-in-tests`, which `--determinism` keeps set); no `allow`/`expect` of `unwrap_used` anywhere (`ESCAPE`, #961) (#195, #962) |
| Component completeness | `tools/lint -- --components` | every first-class component has all 4 axes (field, Add Component entry, inspector card, API namespace), or a written waiver (`WAIVERS`) |
| Editor↔shared-op parity | `tools/lint -- --parity` | every first-class component's inspector card routes its mutations through a shared `scene::authoring` op (never direct field writes through the #344 accessor guard) |
| Rust API reference | rustdoc | **hard gate**: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` (both feature sets) — no broken intra-doc links, no public docs linking private items; name a private item as plain `code`, not a link |
| Tests | nextest + `cargo test --doc` (`make test`) | **hard gate**: the engine's tests and doctests, both feature sets; the GPU tests run on lavapipe (`docs/testing.md`) |
| Supply chain | cargo-deny (`make deny`) | **hard gate**: advisories, bans, sources and licenses (`deny.toml`) |
| Local gate = CI gate | `unittest` (`make scripts`, `test_gate_mirror.py`) | **hard gate**: every command a `GATES` recipe runs appears in a step of `ci.yml`/`lint.yml`, and every clippy run passes nothing after `--` but `-D warnings` (#964) |
| Coverage floors drop with a reason | `unittest` (`make scripts`, `.github/scripts/coverage_floors.py`) | **hard gate**: a floor in `coverage-baseline.txt` lowered from the merge base with `origin/main` needs an inline `# reason: <why>` on its line, and a removed floor passes only once its `src/<module>/` is gone; the weekly coverage run stays a signal (#963) |
| One dependency graph | `make feature-split` (`.github/scripts/feature_split.py`) | **hard gate**: no dev-dependency turns on a feature the normal build lacks, so `cargo test` never rebuilds a shared crate (#552, #979; `docs/testing.md`) |
| Dead `pub` items | `make dead-pub` (`tools/lint -- --dead-pub`) | **signal**, never a gate: `src/lib.rs` exports every module, so rustc's `dead_code` never fires on a `pub` item. On a copy under `target/dead-pub/` the modules go `pub(crate)` and every bin and the integration suite fold into the lib; the report lists items nothing uses, items only tests use (a test seam may be fine), and unused imports apart (#940, #1003) |
| Docs name what exists | `make docrefs` (`.github/scripts/docrefs.py`) | **hard gate**: every `src/`, `tests/`, `tools/`, `docs/` or `.github/` path in a tracked `*.md`'s inline code is tracked, every `` `make <verb>` `` is a target, and CLAUDE.md's namespaces each have a `docs/api/` page; placeholders pass, anything else takes `<!-- docrefs: allow <why> -->`. Runs on Markdown-only PRs too (#973) |

## The `*_tests.rs` sibling rule
The tight test cap exists to discourage over-splitting a bundle of `#[test]`s. But a
`<x>_tests.rs` file sitting next to its `<x>.rs` source is the *opposite* — it is that
one source's dedicated, single test home. Forcing it under the 150-line cap pushes the
overflow back into an inline `#[cfg(test)] mod tests`, so the same source ends up tested
from *two* places. To keep cohesion, a `<x>_tests.rs` **sibling** shares its source's
300-line cap (source + sibling read as one logical unit); the standalone `*_test.rs`
(singular), `tests/`, and `fixtures/` forms keep the tight 150-line cap (#211).

The source has to be there (#960): the gate grants the 300 cap only when `<x>.rs` or a
`<x>/` module directory sits beside `<x>_tests.rs`. A `_tests.rs` with no source of
that name — a second test file mounted with `#[path]`, a `gpu_tests.rs` testing its
directory's `mod.rs` — is a standalone test file at 150. Past that, merge it into the
source's real sibling, give it a sibling (split the code it tests into `<x>.rs`), or
split it by responsibility.

## What the size gate scans
The full scan (`make size`, CI's `lint` job) walks `src/`, `tests/`, `fuzz/`,
`tools/`, `.github/` (the merge scripts, #926) and `engine/` (the shipped shaders and
script components, #1015) — `SIZE_ROOTS` in `tools/lint/src/size/mod.rs`. `assets/` is
not walked: it is gitignored, the open project's seeded and baked copies. Until #535 it walked only `src/`,
so oversized integration-test files passed unseen, and until #541 it skipped `tools/`,
so the lint crate itself could drift past the cap. Any `target/` directory (cargo
<!-- docrefs: allow build output, never tracked -->
build output, e.g. `tools/lint/target/` or `fuzz/target/`) is never descended into.
The commit hook checks whatever `.rs`, `.py`, `.lua` and `.wgsl` files are staged, wherever they live.

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
  (and `tools/lint`'s, when a staged file is under it) and the size gate on the staged
  `.rs`, `.py`, `.lua` and `.wgsl` files — nothing else, so it stays under a
  second. Activate it once per clone with `make setup` (cloud sessions do it in
  `.claude/hooks/session-start.sh`). `git commit --no-verify` skips it; CI doesn't.
  `make setup` also switches the worktrees' compile cache on when sccache is
  installed (`docs/testing.md`, *The compile cache*).
- **Push hook:** `.githooks/pre-push` → `make resolved` (#1096): rustfmt over every
  crate the pushed branch changes against `origin/main`, `tools/lint` included, with
  no build. It exists because `git rebase --continue` runs no commit hook, so a
  conflict resolved by hand would otherwise reach CI unformatted. It checks the
  working tree, so only a pushed tip that is `HEAD`; `git push --no-verify` skips it.
- **Before readying a PR:** `make gates`.
- **CI:** `.github/workflows/{ci,lint}.yml` — the durable layer; survives `--no-verify`.

## The lint policy lives in `Cargo.toml`
Crate-wide lint levels go in `Cargo.toml`'s `[lints]` table, never in command-line
flags: a plain `cargo clippy`, rust-analyzer and CI then all read the same policy,
so nothing is green locally and red in CI. CI adds only `-D warnings`. Thresholds
stay in `clippy.toml`. Module-scoped rules stay module-scoped — the sim modules'
`#![deny(clippy::unwrap_used)]` below is deliberately not crate-wide (the
`unwrap-deny` rule derives its scope from the layer table). `tools/lint`
is held to the same policy (#935): `make clippy` and CI clippy it with `-D warnings`,
and its own `[lints]` carries the nothing-temporary denies.

## What the size gate measures, and nothing grandfathered (#926)
The size gate walks `src/`, `tests/`, `fuzz/`, `tools/`, `.github/` and `engine/`, and
measures four languages. A `.rs` file counts every line. A `.py` file counts its **lines of
code**: blank lines, `#` comments and docstrings are free, as scorsese's gate counts
them, because a Python file's prose grows with how carefully it explains itself and
the merge scripts do that on purpose. A string that is data (`X = """…"""`) is code.
A docstring is recognised as a triple-quoted string that opens a line; a
triple-quoted *argument* opening a line inside a call reads as free too, a limit
the scripts never hit. The violation line says `(lines of code)` so nobody compares
it with `wc -l`.

The merge path in `.github/scripts/` is why `.py` is measured: it is the most
agent-edited code outside `src/`, and `merge-queue.py` had reached 1,074 lines
before the gate could see it. It is now `.github/scripts/merging/`, one
responsibility per module.

`.lua` and `.wgsl` count lines of code the same way (#1015): Lua's `--` and
`--[[ … ]]` comments are free (a long string `[[ … ]]` is data, so code), and so are
WGSL's `//` and nested `/* … */` comments, which carry a shader's maths. The engine's
shaders and script components are edited by agents like any other code: before
#1015 `shader.wgsl` was 462 lines of code and `input_field.lua` 306.

A shader splits through **naga_oil modules**, since WGSL has no `#include`. A module
names itself with `#define_import_path` (`forward::bindings`, `forward::lighting`,
`sky::noise`) and is listed in `shadergen::compose::MODULES`, in dependency order,
so every engine shader, bake and preview composes against it. **No name ever leaves
`common`**: a baked shader on disk imports from it by name, so only a helper common
alone calls can move out (`sky_fbm`). A shader split is proven pixel-identical against
the GPU test suite's readbacks before and after, never blessed.

**Nothing is grandfathered.** A file over its cap is split by responsibility, never
excused: there is no baseline file, for the size gate or any other. The size,
component and parity burn-down lists had all emptied, and an empty list is
machinery for an exception that no longer exists, so #926 retired them. An exception
a check does need is written inline beside what it excuses, with its reason (the
component `WAIVERS`, the rules gate's `lint: allow`), never in an unread list.

`.lua` and `.wgsl` are not measured yet.

## Rules, findings and inline allows (`--rules`)
`--rules` runs every check built on one shared shape (#934): a rule reports
`path:line: [rule] message`, and code that breaks a rule on purpose says why with an
inline comment, on the flagged line or in the comment/attribute lines right above it:

```
// lint: allow(prefix-group) the three mirror Unity's NavMesh* component names
```

The reason is required; an allow without one is itself a finding (`[allow]`). A new
rule is one more call in `tools/lint/src/rules/mod.rs`: no new flag, gate, CI step
or baseline.

- **`prefix-group`** — CLAUDE.md's "group by subfolder, not by filename prefix".
  Three or more sibling `.rs` files whose names start with the same word (up to the
  first `_` or `-`) fail. Files count by stem, so `x.rs` and `x_tests.rs` are one,
  and a bare `nav.rs` counts with the `nav_*` files beside it; `mod.rs`, `main.rs`
  and `lib.rs` never count. The finding anchors on the first `mod` line that
  declares the group, which is where its allow goes. The flat registries CLAUDE.md
  exempts (`src/api/`, one file per Lua namespace; `src/components/`, one file per
  first-class component) are listed in `rules/prefix.rs` with the reason, and the
  exemption covers that directory only, not its subfolders.
- **`markers`** — nothing in rusty is temporary. A marker word (`TODO`, `FIXME`,
  `XXX`, `HACK`) standing alone in a scanned `.rs` file fails, comment or not;
  part of a longer identifier (`TODOS`, `MY_HACK`) does not. File an issue instead.
- **`skip`** — an ignored test reads as passing to anyone who reads only the
  summary. `#[ignore]` and `#[cfg_attr(…, ignore)]` fail unless an allow says why:
  `// lint: allow(skip) a timing measurement, run by hand` right above it.
- **Test code**, for every rule below that says it, is one classifier
  (`rules/test_code.rs`, #1068): all of `tests/`, every `tests.rs` / `*_tests.rs`,
  and a file's lines from its first `#[cfg(test)]` on, scanning only `src/` and
  `tests/`.
- **`test-inner-cfg`, `test-user-workspace`, `test-gpu-entry`** (#980) — three
  `docs/testing.md` locks on test code; comment lines never count. A `tests/` file never gates itself with an inner `#![cfg(…)]` (gate
  its `mod` line); test code never calls `in_user_workspace` or `scenario::run(`
  (use `scenario::run_isolated`); and test code reaches a GPU only through
  `render::test_gpu::headless_or_skip`, never `new_headless` directly
  (`rules/test_shape.rs`).
- **`full-literal`** (#976) — a test builds a first-class component (a `…Component`
  type `Entity` names, with three or more fields) through a constructor or a
  `..rest`, never a struct expression naming every field: such a literal breaks the
  day the component gains a field. When every field is the point, say so:
  `// lint: allow(full-literal) <why>` (`rules/full_literal.rs`).
- **`ignored`** — what you push must be what CI builds. A scanned `.rs` file that
  `git check-ignore --no-index` matches fails, tracked or not, because the next
  new file beside it is the one a push silently drops
  (MatthewLacerda2/scorsese#872). It takes **no allow**: anchor the pattern
  (`/out/`, not `out/`) or move the file. A failed `git` call is a finding too.
- **`unwrap-deny`** — the unwrap ban covers exactly the layer table's sim rows
  (#962). Each sim row's `src/<module>/mod.rs` must carry
  `#![deny(clippy::unwrap_used)]`, and no file under a sim module may `allow`,
  `expect` or `warn` `clippy::unwrap_used` (or the `clippy::restriction` group).
  A new sim row is covered the day it is added. Platform files are not scanned.
- **`test-temp-dir`** — a path under `std::env::temp_dir()` is shared by every test
  process, every worktree and (under `cargo test` and cargo-mutants) every test
  thread, so a fixed or process-keyed name races. Test code under `src/` and
  `tests/` calls `crate::test_temp::dir()` (in-crate) or `crate::temp::dir()`
  (integration) instead: both key on the process *and* the running test (#709,
  #999). Comment lines never count. The
  helper itself and Unix socket paths (which must stay short) carry an inline
  allow. `tools/` is not scanned: the lint crate's own tests are not the engine's.
- **The ≲10-files-per-directory signal** prints after the findings: every scanned
  directory holding more than ten `.rs` files (`mod.rs` aside), most crowded first.
  CLAUDE.md says "aim for", so it never fails the gate; it tells you where to
  cluster by responsibility when you next work in that directory.

## Module layering (`--layers`)
Which module may import which is declared once, in `tools/lint/src/layers/table.rs`
(#724). It replaced 64 per-file `//! Allowed deps:` headers that nothing checked
(at least 7 had drifted) and the copies of the sim-module list that the determinism
and direction guards, `CLAUDE.md` and these docs each kept.

- **One row per `src/<module>`, bottom-up.** A row names the modules it may import,
  and each must sit **above** it. The table is then a topological order: the graph
  has no cycles. The only pair allowed to reference each other is listed in `PEERS`
  (`scripting` ↔ `api`: scripting installs the Lua surface, the surface reads state
  scripting owns).
- **Rows mark the sim.** Everything a sim module imports must be sim as well, so
  `scene → shadergen → render` style leaks fail, not only direct ones. The
  direction guard scans this derived set (`layers::sim_dirs`), the determinism gate
  lets only the other rows opt out of the clock ban (`layers::platform_roots`), and
  `test-lint` pins the sim set, so a module leaves the sim only through a reviewed edit.
- **Exact, not a ceiling.** The lint measures every non-comment `crate::<module>`
  reference outside test code and reports `UNDECLARED_DEP` (code uses an edge the
  row lacks), `STALE_DEP` (the row lists an edge nothing uses), `CYCLE`, `SIM_LEAK`,
  and `UNDECLARED_MODULE` / `MISSING_MODULE`. Test code (`#[cfg(test)]` items and
  test-only module files) is exempt, as in the other guards.
- **Known exceptions** go in `EXCEPTIONS`, each naming the open issue that removes
  it; the lint reports `STALE_EXCEPTION` once the edge is gone. The list is empty:
  #737 and #738 removed the two the lint landed with. A layer above the sim that adds
  to the Lua surface does it through `api::extend`, as `dev` does for `Debug.*` and
  the bake verbs, so the edge runs down, not up.

A new import between modules is one edit to the row, in the same PR. If the edge
breaks the order or leaves the sim, move the code down a layer instead.

### External crates (#938)
Each row also names the external crates its module may use (`.uses(&[…])`, the
crate name as code writes it: `egui_wgpu`, not `egui-wgpu`), so `wgpu` stays in
`render`, `winit` in `shell`, `kira` in `audio`, `mlua` in `scripting`/`api`/`dev`.
What is not one row's lives in `tools/lint/src/layers/externs/policy.rs`:

- **`common`** — the vocabulary any module may name unlisted: `glam`, `serde`,
  `serde_json`, `log`.
- **`unscoped`** — dependencies no module names, each with why it is in
  `Cargo.toml` (`env_logger`: the binaries; `getrandom`: a feature pin, #552).
- **`confined`** — crates only some subtrees may name, finer than a module: the
  `egui` crates only under `src/editor` and `src/shell/editor` (the `editor`
  feature, #431), the provider crates only under `src/dev/providers` (#383).
- **`exceptions`** — a use the rules forbid but the code still has, each naming its open issue. There are none today.

The lint reads `Cargo.toml`'s `[dependencies]` (and per-target ones) and reports
`UNDECLARED_CRATE` (a module names a crate its row lacks), `STALE_CRATE`,
`CONFINED_CRATE`, `UNCLAIMED_CRATE` (a dependency no row, `common` or `unscoped`
accounts for, so a **new dependency** needs a table decision in the same PR),
`UNKNOWN_CRATE`, `REDUNDANT_CRATE` (a row listing a common crate) and
`STALE_EXCEPTION`. It counts the first segment of a `use` path and any `name::`
path elsewhere, outside comments and test code. A file that declares `mod image;`
or imports a local `image` means its own module by `image::`, so that name is not
counted there; glob imports are not followed, the one place a local name could
pass for a crate.

## Sim determinism (clippy + `--determinism`)
The sim is a pure function of (seed, inputs, fixed dt), so a wall-clock read or an
unseeded RNG in it breaks replay for every harness run. **Clippy holds the ban**
(#757): `clippy.toml` lists `Instant`, `SystemTime`, their `now`/`elapsed`,
`getrandom`'s entry points and `rand`'s global RNG under `disallowed-methods` /
`disallowed-types`, each with the reason clippy prints, and `Cargo.toml`'s `[lints]`
denies both. Clippy matches the **resolved** item, so `use std::time::Instant as
Clock; Clock::now()` fails too — the old substring scan let that through.

`std::collections::{HashMap, HashSet}` and `std::hash::RandomState` are on the same
list (#764): std seeds its hasher from OS entropy per process, so a map's iteration
order is an unseeded RNG that never shows up as a call. Sim code uses
`core::collections::{Map, Set}` (FxHash, no seed; build with `Map::default()`), and a
`BTreeMap` where the order is behaviour — what is stepped, spawned, emitted or
serialised first. `tests/busy_replay` replays a busy scene in one process and across
two, which is where a leaked order would show.

The ban is crate-wide. The platform modules that need real time (`dev`, `editor`,
`render`, `shell` today) opt out with
`#![allow(clippy::disallowed_methods, clippy::disallowed_types, reason = "…")]` at
their `mod.rs`. `make determinism` keeps that escape hatch honest: it fails
(`OPT_OUT`) when any other file under `src/` names those lints or allows the groups
that hold them (`clippy::style`, `clippy::all`), so a sim module can't quietly gain
an `allow`; and it fails (`UNBANNED` / `UNDENIED`) when `clippy.toml` drops a banned
item or `Cargo.toml` stops denying the lints. Which rows are platform comes from the
layer table. The same `disallowed-methods` list bans process-wide state
(`std::env::{set_var, remove_var, set_current_dir}`, #977), which a sim module may
own for one statement: that statement's `#[expect(clippy::disallowed_methods, …)]`
sits under `// lint: allow(statement-opt-out) <why>` (`project::open`), and
`--determinism` accepts it there and nowhere else. Test code in a sim module is not exempt: a sim test has no reason to
read the clock either. The integration suite (`tests/`) is outside the gate's scan,
like test code in the other guards; a test that times a bake allows the lints on
its own module (`tests/navigation_layered/mod.rs`).

## Dependency direction (`--direction`)
The sim runs headless with no GPU and no UI, so the dependency arrow points one way:
`render` and `editor` import sim types, never the reverse. `tools/lint -- --direction`
scans the table's sim modules and fails on any non-comment reference
to `crate::render`, `crate::editor`, `wgpu` or `egui` (whole path segments only).
The layering lint already covers the `crate::` half, transitively; this guard keeps
the external crates, which the table does not model.
Plain data the renderer and the sim share lives sim-side: the mesh `Vertex` and the
primitive builders in `components::mesh`, `Camera` and `Decal` in `scene`,
`QualityPreset` in `core::quality`; the renderer keeps only the GPU half (e.g.
`render::gpu::mesh::vertex_layout`). There is no baseline — the guard landed with
zero violations (#494), and it is the groundwork the crate split (#495) needs.
`api` joined the scan in #723: it is the surface scripts drive from inside the sim.
`shadergen` joined in #722, once its GPU-free `naga_oil` composition moved out of
`render` into `shadergen::compose` (the renderer's `ShaderRegistry` now calls it).

## Component completeness (`--components`)
A first-class component is only "done" when it appears on all four axes that
deliberately live in non-dependent layers: a field on `Entity`, an Add Component
entry (`inspector/components/add/`: a `!world.has_<field>(id)` gate, or a
`ComponentKind` row, resolved to its field through `authoring::has_kind`'s arms), an inspector card (some `inspector/components/*.rs`), and an API
namespace (`src/api/<x>.rs` registered in `api/mod.rs` and documented in
`docs/api/`, as the field or its Lua spelling: `layout_group` is `LayoutGroup`). The
gate discovers components from `Entity`'s `Option<…Component>` fields — so a new one
can't slip through — and fails on any missing axis. It *finds* `pub struct Entity`
under `src/components/` rather than reading a fixed path, and fails loudly
(`NO_ENTITY` / `NO_COMPONENTS`) when it can't or when the struct yields no
component fields: for weeks after `entity.rs` became `entity/mod.rs` both gates
checked zero components and printed ok (#959). A test-lint test runs discovery on
the real tree. Nothing is grandfathered: the burn-down list emptied under #82 (every
gap closed or waived) and was retired in #926, so a missing axis is closed or waived.

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
  `collider api` + `rigidbody api` (→ `Physics`), `nav_agent api`
  (→ `NavMeshAgent`), `visual_correction api` (→ `Graphics`).

**`WAIVERS`** is for axes we have decided not to implement standalone, never for an
axis not closed yet: that one fails until it is. A row that no longer waives anything
fails too (`STALE_WAIVER`): its component is no longer discovered, or the axis is
present after all (`material api` was, once `src/api/material/` landed), so a
waiver can't outlive its reason and hide a later regression. A waiver is never a silent skip — its rationale lives in code and is reviewable in
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

The detection signal is facade-shaped (#344) and judged per card file: a file that
takes `.<field>_mut(` and imports no `scene::authoring` ops marks a direct card,
whatever the ops module is called (`nav::agent` serves `nav_agent`, `backdrop`
serves `backdrop_filter`). Coarse substring scan, matching the other gates.

### Routed vs direct (#287)
Every card is routed: the burn-down list of direct cards emptied under #287 and was
retired in #926. Enforcement:

- A **direct** card → **violation** (drift: a direct mutation was added or kept).
- A **routed** card → ok. This is the gate's **teeth**: reverting `material` (or any
  card) to a direct mutation fails the build.

This is a **hard gate** (CI `--parity` step, in the lint workflow): correctness of the
no-drift invariant, green-to-merge. A new direct card is fresh drift to fix by routing
it through a shared op, never to grandfather.

## The function-length cap (`too_many_lines`)
The 50-line per-function cap is a **hard clippy gate** (`too_many_lines = "deny"` in
`Cargo.toml`'s `[lints.clippy]`, both feature sets) and is **on for the whole crate**
— the legacy functions that predated it were split (#124). The one remaining
exception is `Renderer::assemble` in `src/render/setup/mod.rs`
(`#[expect(clippy::too_many_lines, reason = …)]`), a flat one-line-per-field
constructor whose length is the struct's width. If a function grows past 50 lines,
split it: `make determinism` fails (`ESCAPE`) on any other `allow`/`expect` of
`too_many_lines`, and on any of `unwrap_used` (`ESCAPES` in
`tools/lint/src/determinism/escapes.rs`, #961).

Every sim module (the layer table's `sim` rows) carries a module-level
`#![deny(clippy::unwrap_used)]` at its `mod.rs`, held there by the rules gate's
`unwrap-deny` (#962), so a bare `.unwrap()` in
their **production** code is a hard clippy error (caught by the same `-D warnings`
gate, both feature sets, `make gates` + CI). This is the **survivability**
sibling of the determinism guard's **reproducibility**: a `.unwrap()` that panics
mid-frame kills an unattended/overnight play-test just as surely as a wall-clock
read breaks replay — same rationale (#195). The ban began in five modules (`app`, `scripting`,
`physics`, `navigation`, `ui`) and widened to every sim row in #962, once the 10
unwraps it found (all in `scene`'s default scene and material authoring) handled
their missing case.

The rule is deliberately narrow:
- **`unwrap`, not `expect`.** `.expect("clear invariant")` is the sanctioned escape
  hatch — it documents *why* the value must be present at the call site. Use `?`
  where a `Result` should propagate, `.expect(...)` where the invariant is real and
  local; reach for the bare `.unwrap()` nowhere in the sim core.
- **Sim only.** The platform layer (the table's non-sim rows) is exempt —
  e.g. `render/gpu/shaders.rs` panicking at boot on a bad shader is fail-fast-at-startup,
  not a mid-sim hazard. The determinism guard's exemption is the same platform layer.
- **Tests exempt.** `clippy.toml`'s `allow-unwrap-in-tests = true` lets test code
  unwrap freely, so no per-test `#[allow]` noise.

There is no baseline/burn-down: each widening fixed every site first, so the lint
went straight to `deny` with no grandfathered sites. Keep it that way — fix the call
site; an `allow`/`expect` of `unwrap_used` fails `make determinism`.

## Clippy: CI vs local
Clippy lints the whole crate (it can't be scoped to changed files), which is too
slow for a commit hook — so the hook doesn't run it. `make gates` and CI run the
same hard clippy gate (`-D warnings`, both feature sets); CI is the durable one that
gates the PR.
