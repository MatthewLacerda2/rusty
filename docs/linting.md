# Linting & the commit gate

Programmatic, no AI in the loop. `make gates` runs every check below; the commit
hook runs the fast ones. `tools/lint` writes its result to `.lint/report.txt` so an
agent can read exactly what failed.

## What's enforced
<!-- sorted: each check is one row, so two branches adding a rule land on different lines (#1409) -->
| Check | Tool | Rule |
|---|---|---|
| Active in the hierarchy | `tools/lint -- --rules` (`active-self`) | **hard gate**: outside `src/ecs/`, no non-test line of `src/` calls `is_active_self(` — systems ask `World::is_active_in_hierarchy`, so an inactive parent hides its subtree; what shows or stores the own flag takes `// lint: allow(active-self) <why>` (#1363) |
| Asset writes tell the caches | `tools/lint -- --rules` (`asset-write`) | **hard gate**: a non-test line of `src/` outside `src/core/fs/` that calls `fs::write(`, `File::create(` or `.save(<arg>)` has a `note_written` later in its function, or `// lint: allow(asset-write) <why>` (a file no path-keyed cache reads) (#1220) |
| Code smells | clippy (`clippy.toml`) | **hard gate**: `cargo clippy --all-targets -- -D warnings` (default, `--features dev`, and `--no-default-features` — the editor-free player build), plus `tools/lint` itself (#935) |
| Component completeness | `tools/lint -- --components` | every first-class component has all 4 axes (field, Add Component entry, inspector card, API namespace), or a written waiver (`WAIVERS`) |
| Converters report, never log | `tools/lint -- --rules` (`converter-log`) | **hard gate**: no code line of a converter under `src/asset/convert/` (every module, folder or single file, but the cache's own `mod`, `converter`, `depends`, `gate`, `lock`, `record`, `stage`, `stale`, `texture_ref` and `tests`) names `log::` or a bare logging macro — a converter returns what it skipped in `Converted` and fails on anything else, and `convert::refresh` alone warns; each converter's root carries `#![deny(clippy::let_underscore_must_use, clippy::unused_result_ok, clippy::print_stdout, clippy::print_stderr)]`, wrapped or not, so clippy refuses a dropped `Result` and a print there (#1172) |
| Coverage floors drop with a reason | `unittest` (`make scripts`, `.github/scripts/coverage_floors.py`) | **hard gate**: a floor in `coverage-baseline.txt` lowered from the merge base with `origin/main` needs an inline `# reason: <why>` on its line, and a removed floor passes only once its `src/<module>/` is gone; the weekly coverage run stays a signal (#963) |
| Dead `pub` items | `make dead-pub` (`tools/lint -- --dead-pub`) | **signal**, never a gate: `src/lib.rs` exports every module, so rustc's `dead_code` never fires on a `pub` item. On a copy under `target/dead-pub/` the modules go `pub(crate)` and every bin and the integration suite fold into the lib; the report lists items nothing uses, items only tests use (a test seam may be fine), and unused imports apart (#940, #1003). It builds for the host only, so a finding in a file with a platform `cfg` (or in a module declared under one) is listed under *Platform-gated*: check it on that platform before narrowing (#1092). It runs weekly on `main` as `cargo-test.yml`'s `dead-pub` job, the report in its job summary; the job goes red only when no report came out, and `main-health` files that (#1205) |
| Dependency direction | `tools/lint -- --direction` | the table's sim modules never reference `crate::render`, `crate::editor`, `wgpu` or `egui` — the arrow is render/editor → sim (#494) |
| Dependency locks | cargo-deny (`deny.toml`) | **hard gate**: every git dependency is pinned by `rev` (`required-git-spec`); `nalgebra` only through the physics stack and no `cgmath` — glam is the one math crate (#975) |
| Directory size | `tools/lint -- --rules` (`dir-size`) | **hard gate**: no directory under `src/` holds more than 12 source files directly in it (tests and its `mod.rs` aside); cluster by responsibility into subfolders, or a flat registry of one file per public unit says so with an inline `// lint: allow(dir-size) <why>` in its `mod.rs` (`src/api/`). The ≲10 aim prints as a signal (#1367) |
| Docs name what exists | `make docrefs` (`.github/scripts/docrefs.py`) | **hard gate**: every `src/`, `tests/`, `tools/`, `docs/` or `.github/` path in a tracked `*.md`'s inline code is tracked, every `` `make <verb>` `` is a target, and CLAUDE.md's namespaces each have a `docs/api/` page; placeholders pass, anything else takes `<!-- docrefs: allow <why> -->`. Runs on Markdown-only PRs too (#973) |
| Editor↔API parity per op | `tools/lint -- --rules` (`api-parity`) | **hard gate**: every `scene::authoring::<module>::set_*` op a non-test line of `src/editor/` names is also named from `src/api/` — a named setter's call or an `op!`/`with!`/`group!` row of `api::component`'s tables — so anything the inspector writes, Lua can; an editor-only op takes `// lint: allow(api-parity) <why>` at its `fn`, and an allow on an op Lua reaches or no inspector names is stale and fails (#1368) |
| Editor↔shared-op parity | `tools/lint -- --parity` | every first-class component's inspector card routes its mutations through a shared `scene::authoring` op (never direct field writes through the #344 accessor guard) |
| Entity ids from Lua | `tools/lint -- --rules` (`entity-arg`) | **hard gate**: in `src/api/`, a Lua callback never takes an entity (`id`, `…_id`, `parent`, `child`, `target`, `entity`, `body`, `canvas`) as a bare `u32` or `Option<u32>`: it takes `api::entity_arg::EntityArg`, so a `nil` id reads as a missing entity and fails a write with the verb's name (#1383) |
| File length | `tools/lint` | <= 300 lines: every line of a `.rs` file, the lines of code of a `.py`, `.lua` or `.wgsl` one (docstrings, comments and blank lines free, #926, #1015); nothing grandfathered |
| Finite authoring floats | `tools/lint -- --rules` (`sanitize`) | **hard gate**: every `pub fn set_*` under `src/scene/authoring/` that takes an `f32` or a glam vector passes it to a `scene::authoring::sanitize` helper (or a sibling setter of its file), so NaN and `±inf` from Lua or the editor take the field's fallback and never reach a field; a value checked another way takes `// lint: allow(sanitize) <why>` (#1361) |
| Folder grouping | `tools/lint -- --rules` | **hard gate** (`prefix-group`): no three sibling `.rs` files share a first word (`nav_agent.rs`, `nav_obstacle.rs`, …); make it a subfolder and drop the prefix (#934) |
| Full component literals in tests | `tools/lint -- --rules` (`full-literal`) | **hard gate**: test code builds a first-class component with `..rest` or a constructor, never a literal naming every field (#663, #976) |
| Function ("endpoint") length | clippy `too_many_lines` + `tools/lint -- --determinism` | **hard gate**: `too-many-lines-threshold = 50` (`clippy.toml`), denied crate-wide in `Cargo.toml`'s `[lints]`; its one exception, `Renderer::assemble`, is locked (`ESCAPE`, #961) |
| Lint exceptions carry a reason | clippy `allow_attributes` + `allow_attributes_without_reason` | **hard gate**: an exception is `#[expect(lint, reason = "…")]` (outer `#[allow]` is denied; a module-root `#![allow]` still needs its `reason`), and an expectation that no longer fires fails too (#961) |
| Local gate = CI gate | `unittest` (`make scripts`, `test_gate_mirror.py`) | **hard gate**: every command a `GATES` recipe runs appears in a step of `ci.yml`/`lint.yml`, and every clippy run passes nothing after `--` but `-D warnings` (#964) |
| Lua paths are `/` | `tools/lint -- --rules` | **hard gate** (`lua-path-slash`): a path returned to a script goes through `core::paths::to_slash`, never `to_string_lossy()`/`display()` (#1390) |
| Merge helpers | `unittest` (`make scripts`) | **hard gate**: `.github/scripts/tests` pass — `make mergeable` is the only thing between a red PR and `main` while it has no required checks (#486, #491) |
| Module layering | `tools/lint -- --layers` | every `crate::<module>` import and every external crate a module names matches the declared table (`tools/lint/src/layers/table.rs`): nothing undeclared, nothing stale, no cycle, no sim → platform edge (#724, #938) |
| No shared process state | clippy `disallowed_methods` + `tools/lint -- --determinism` | **hard gate**: `std::env::{set_var, remove_var, set_current_dir}` are banned crate-wide in `clippy.toml`, tests included, since `cargo test` runs every in-crate test in one process; production opts out inline with a reason (`project::open`), and `--determinism` keeps the three on the list (#977) |
| No subprocesses | `tools/lint -- --rules` (`subprocess`) | **hard gate**: no line of `src/` names `std::process::Command` — the engine starts no other program but Blender, to convert a `.blend` while authoring; a real need takes `// lint: allow(subprocess) <why>` (#975) |
| Nothing temporary | clippy `dbg_macro`/`todo`/`unimplemented` + `tools/lint -- --rules` (`markers`) | **hard gate**: the three are denied in `Cargo.toml`'s `[lints]` (and `tools/lint`'s); a marker word (`TODO`, `FIXME`, `XXX`, `HACK`) in a scanned `.rs` file fails — an unfinished thing is an issue (#935) |
| One atomic write | `tools/lint -- --rules` (`atomic-write`) | **hard gate**: no line of `src/` outside `src/core/fs/` names `fs::rename` — a file a reader may open is written through `core::fs::write_atomic`; a rename that moves rather than writes takes `// lint: allow(atomic-write) <why>` (#1129) |
| One dependency graph | `make feature-split` (`.github/scripts/feature_split.py`) | **hard gate**: no dev-dependency turns on a feature the normal build lacks, so `cargo test` never rebuilds a shared crate (#552, #979; `docs/testing.md`) |
| One fullscreen triangle | `tools/lint -- --rules` (`fullscreen-triangle`) | **hard gate**: no `.wgsl` under `engine/` but `fullscreen.wgsl` spells the triangle's `(i << 1u) & 2u`; `#import fullscreen` and call `fullscreen::triangle` / `uv` (#1370) |
| One name table per enum | `tools/lint -- --rules` (`named-enum`) | **hard gate**: no file of `src/` hand-writes both a parse (`fn parse(_: &str) -> Option<Self>` in an `impl T`, or a free `fn parse_*(_: &str) -> Option<T>`) and a name (`fn name`/`fn as_str(self) -> &'static str`, or a free `fn *_name(_: T) -> &'static str`) for one type, nor a `const`/`static` table of `(T, &str)` pairs — a script-facing enum declares its names once with `core::named_enum!`, which writes `ALL`, `name`, a case-insensitive `parse` and `Display`; a parse that does more takes `// lint: allow(named-enum) <why>` (#1329) |
| One world space | `tools/lint -- --rules` (`world-space`) | **hard gate**: outside `ecs::world_space` and `components::transform`, no engine code line decomposes a matrix (`to_scale_rotation_translation`), names the retired `compute_world_matrix` / `world_matrix(` / `parent_world_matrix`, or folds a Transform over `ancestors(` — world space is read from `ecs::World`'s world API; test code is not scanned, and converting file data takes `// lint: allow(world-space) <why>` (#1196) |
| Order-free lists stay sorted | `tools/lint -- --rules` (`sorted`) | **hard gate**: a list under a `// sorted: <why>` (or `<!-- sorted: <why> -->`) marker stays in order up to its first blank line, so two branches adding entries touch different lines (#911) |
| Portable paths | `tools/lint -- --rules` | **hard gate** (`slash-join`): `src/` and `tests/` never join a `/`-separated path (literal, `format!` or constant) onto a path or make a path from one; they join one part per call or join `core::paths::native(…)`, and never `format!` a native path (`.to_string_lossy()`, `.display()`) before a `/`: that is `core::paths::to_slash` (#1332, #1371, #1428) |
| Pushed = built | `tools/lint -- --rules` (`ignored`) | **hard gate**: no scanned `.rs` file may match a `.gitignore` pattern, tracked or not; no allow (#935) |
| Ranged editor drags | `tools/lint -- --rules` (`drag-range`) | **hard gate**: a `DragValue` under `src/editor` bounds its range with `.ranged(..)` (`editor::widgets::DragRange`), never egui's `.range(..)`, which also rewrites an out-of-range value just by selecting the entity (#1158); an editor slider is built by `editor::widgets::slider`, never a bare `egui::Slider::new(`, whose default clamping does the same (#1444); a real need takes `// lint: allow(drag-range) <why>` |
| Rust API reference | rustdoc | **hard gate**: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` (both feature sets) — no broken intra-doc links, no public docs linking private items; name a private item as plain `code`, not a link |
| Shared process | `make test-one-process` (`cargo test --features dev --tests`) | **signal**, never a gate: the `dev` suite with every test of a binary in one process, as cargo-mutants runs it, so a test leaking process-wide state (a moved working directory, a `static` left set) is named before it breaks the mutation baseline. Nightly on `main` in `cargo-test.yml` (both feature sets), filed by `main-health` when red; `gh workflow run cargo-test.yml --ref <branch>` runs it on a branch (#1338) |
| Sim determinism | clippy `disallowed_methods`/`disallowed_types` + `tools/lint -- --determinism` | **hard gate**: clock reads, OS/unseeded RNG and `std`'s randomly seeded hash maps are banned crate-wide in `clippy.toml`; only a platform row's `mod.rs` may opt out, and the ban list must stay whole (#757, #764) |
| Sim panic-freedom | clippy `unwrap_used` + `tools/lint -- --rules` (`unwrap-deny`) | **hard gate**: every sim row of the layer table carries `#![deny(clippy::unwrap_used)]` at its `mod.rs`, and no sim file lifts it; bare `.unwrap()` banned in production (test code exempt via `allow-unwrap-in-tests`, which `--determinism` keeps set); no `allow`/`expect` of `unwrap_used` anywhere (`ESCAPE`, #961) (#195, #962) |
| Skipped tests | `tools/lint -- --rules` (`skip`) | **hard gate**: `#[ignore]` / `#[cfg_attr(…, ignore)]` needs `// lint: allow(skip) <why>` (#935) |
| Style | rustfmt (`rustfmt.toml`) | `cargo fmt --check` |
| Supply chain | cargo-deny (`make deny`) | **hard gate**: advisories, bans, sources and licenses (`deny.toml`) |
| Tagged engine warnings | `tools/lint -- --rules` (`log-tag`) | **hard gate**: every engine warning or error in `src/` (`log::warn!` / `log::error!`, an imported bare `warn!` / `error!`, `log!` at `Level::Warn`/`Level::Error`) opens its message with its module, `[Asset] …`, since those records reach the console and the session/MCP replies (#1139); a message that is not a literal takes `// lint: allow(log-tag) <why>` |
| Test / fixture file length | `tools/lint` | <= 150 lines (standalone `*_test.rs` / `tests/` / `fixtures/`, a `*_tests.rs` with no source beside it, and every `.py` under a `tests/` directory); a `<x>_tests.rs` **sibling** of `<x>.rs` (or of a `<x>/` module directory) shares the 300-line source cap |
| Test source shape | `tools/lint -- --rules` (`test-inner-cfg`, `test-user-workspace`, `test-gpu-entry`) | **hard gate**: no inner `#![cfg]` in `tests/`, no user workspace in tests, a GPU only through `headless_or_skip` (#980) |
| Test temp dirs | `tools/lint -- --rules` | **hard gate** (`test-temp-dir`): test code writes under `crate::test_temp::dir()` / `crate::temp::dir()`, never a bare `std::env::temp_dir()` (#999) |
| Tests | nextest + `cargo test --doc` (`make test`) | **hard gate**: the engine's tests and doctests, both feature sets; the GPU tests run on lavapipe (`docs/testing.md`) |
| `tools/lint` stays std-only | `make test-lint` | its `Cargo.lock` names one package, itself (#935) |

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
It measures what can be committed (#1373): git lists the tracked files and the
untracked ones `.gitignore` doesn't exclude (`git ls-files --cached --others
--exclude-standard`), so what the engine seeds into a gitignored folder (the UI
kit's scripts and the default shader it copies into the fixture project's
`assets/`) never counts, and a new file is measured before it is added. Where git
can't list them (a tarball with no `.git`) it walks the disk and says so on stderr.
The code gates (rules, layers, determinism, direction, components, parity) keep
walking the disk: they check what cargo compiles, and an ignored `.rs` file is
compiled all the same, which is the file the `ignored` rule flags.
The commit hook checks whatever `.rs`, `.py`, `.lua` and `.wgsl` files are staged, wherever they live.

## Run it
```
make gates     # everything CI blocks on, fastest-failing first
make quick-gates  # the seconds-long gates and self-checks (all but HEAVY_GATES)
make check     # the fast edit loop: formatting, size gate, dev-feature clippy
make help      # every verb, and each gate on its own (make size, make clippy, …)
```
`make gates` starts with two self-checks. **`target-dir`** asks cargo where it
builds and refuses to run when that is outside the worktree: a target dir shared
between worktrees lets one branch's artifacts stand in for another's, so a green
run would not prove *this* branch compiled. **`inventory`** fails when a Makefile
target documented `## [gate]` is missing from the `GATES` list, or the other way
round, so the gate list cannot silently lose a gate. `GATES` is `QUICK_GATES` plus
`HEAVY_GATES` (the Makefile lists them), defined as exactly that and held by
`make scripts`; a cold run passes a cloud tool call's limit, so cloud coders run
`make quick-gates` and then each heavy gate (#1179).

## Where it runs
- **Commit hook:** `.githooks/pre-commit` → `make pre-commit`: `cargo fmt --check`
  (and `tools/lint`'s, when a staged file is under it) and the size gate on the staged
  `.rs`, `.py`, `.lua` and `.wgsl` files — nothing else, so it stays under a
  second. Activate it once per clone with `make setup` (cloud sessions do it in
  `.claude/hooks/session-start.sh`). `git commit --no-verify` skips it; CI doesn't.
  `make setup` also switches the worktrees' compile cache on when sccache is
  installed (`docs/testing.md`, *The compile cache*).
- **Push hook:** `.githooks/pre-push` → `.github/scripts/resolved.py --hook` (#1096):
  `cargo fmt --all --check` and `tools/lint`'s rustfmt check, the pair `lint.yml`
  runs, with no build (the commit hook's own rustfmt cost). It exists because
  `git rebase --continue` runs no commit hook, so a conflict resolved by hand would
  otherwise reach CI unformatted. rustfmt reads the working tree, so the hook checks
  only when a pushed tip is `HEAD` and lets any other push through; `make resolved`
  runs the same two checks on the working tree unconditionally.
  `git push --no-verify` skips it.
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
rule is a module with its own `<rule>_tests.rs`, one line in the sorted `mod` list
and one row in the sorted `RULES` table of `tools/lint/src/rules/mod.rs` (#911): no
new flag, gate, CI step or baseline.

<!-- sorted: each rule is one section, so two branches adding a rule land on different lines (#1409) -->
- **`active-self`** (#1363) — every system read an entity's own `active` flag, so
  the children of an inactive pivot kept drawing and ticking. Unity's
  `activeInHierarchy` is `World::is_active_in_hierarchy`, and it is the one
  question a system asks; `World::is_active_self` (Unity's `activeSelf`) is for
  what shows or stores the flag itself: the inspector checkbox, `Scene.IsActive`,
  the snapshot, and the UI clip walk that folds one level at a time. A non-test
  line of `src/` outside `src/ecs/` that calls `is_active_self(` fails; those
  callers carry an inline allow. The rule is lexical (`rules/active_self.rs`): it
  misses a read of the flag that does not go through that name, which the private
  `ecs::bundle::Core` keeps inside `src/ecs/`.
- **`api-parity`** (#1368) — "anything the editor does, the API does"
  (`docs/game-agent.md`), field by field. `--parity` holds that the inspector writes
  through a shared `scene::authoring` op; this rule holds that Lua reaches the same
  op: every `authoring::<module>::set_*` op a non-test line of `src/editor/` names
  must be named from `src/api/` too, by a named setter (`Physics.SetMass`) or by
  `api::component`'s field tables (`op!("mass", rb::set_mass)`, which `Component.Set`
  dispatches through), so editor and Lua share one validation. Ops are matched by
  path: each file's `use crate::scene::authoring::…` trees give its aliases
  (`rigidbody as rb`, `{self as ops, …}`, `nav::{agent, …}`, a setter imported by
  name and called bare), so the finding names `nav::agent::set_speed` however a file
  spells it. An op only the inspector may write (a display toggle) takes
  `// lint: allow(api-parity) <why>` above its `fn` or trailing it; the rule reads its
  own allows (`Allow::Own`), matching each to the op defined below it (so one in a
  re-exported sub-file such as `particles/render.rs` counts), and one on an op Lua
  reaches, or that no inspector names, fails as stale. A line scan
  (`rules/api_parity.rs`) with two blind spots: a glob import (`use …::rigidbody::*`)
  and an op reached only through a helper in another module. On the API side that
  reads as unreached (call the op from the binding instead); on the **editor side
  the rule fails open**: an op the inspector writes that way is never asked about.
  `scene::authoring::fields` counts as the API side too: #1418 moves
  `Component.Set`'s field table there.
- **`asset-write`** (#1220) — a file a path-keyed cache reads is written through
  `core::fs::written::write`, or its writer calls `note_written` after (#1207).
  A non-test line of `src/` outside `src/core/fs/` calling `fs::write(`,
  `File::create(` or `.save(<arg>)` is a finding unless `note_written` follows in
  the same function. A write nothing caches by path (scene, prefab, project,
  settings, ledger, harness, bench) takes `// lint: allow(asset-write) <why>`. A
  text heuristic: `.save(path)` also matches a project type's own saver, so its
  callers carry the allow too (`rules/asset_write.rs`).
- **`atomic-write`** (#1129) — a file another process (or thread) may read is
  written through `core::fs::write_atomic`: a temp name unique to the process and
  the call, renamed into place, the temp removed on any failure. Hand-rolled copies
  drifted (a fixed `.part` name, a pid-only one, litter on failure), so a line of
  `src/` outside `src/core/fs/` whose code names `fs::rename` fails. A rename that
  moves rather than writes (the import cache's staging swap, the seed's staged
  files) carries an inline allow. Comment text never counts;
  `tests/` and `tools/` are not scanned (`rules/atomic_write.rs`).
- **`converter-log`** (#1172) — a converter that logs a failure and carries on
  returns `Ok`, and the import cache stamps a `.glb` missing a part as current
  (#1168); one that logs a skip is heard once, then never again from the cache. So
  every module under `src/asset/convert/` but `mod`, `converter`, `depends`, `lock`,
  `record` (the cache, the table of converters, what it depends on, its lock and its stamp record, which report),
  `stage` (the swap), `stale` (whether a cache is current), `texture_ref` (resolving a texture reference) and `tests` is a converter's (a new module of the cache's own joins that list), a folder
  (`fbx/`, `blend/`) or a single file, and a code line there naming `log::` or a
  bare `log!` / `warn!` / `error!` / `info!` / `debug!` / `trace!` fails: the
  converter returns the skip in `Converted`, and `refresh` warns of it on the
  conversion and once per process from the cache. A `//` inside a string literal
  (`"file://…"`) does not hide the rest of the line. Each converter's root
  (`<name>/mod.rs` or `<name>.rs`) must carry one inner `deny` naming
  `clippy::let_underscore_must_use`, `clippy::unused_result_ok`,
  `clippy::print_stdout` and `clippy::print_stderr`, however rustfmt wraps it and
  with or without a trailing comment: clippy, not a text scan, then refuses
  `let _ =` on a `Result`, a discarded `.ok()` and a print anywhere in that
  converter. A `drop(result)` or a bare `_ = expr;` gets past both. The clippy
  lints are per converter, not in `Cargo.toml`, because the rest of the crate has
  some eighty best-effort `let _ =` (a cleanup, a send to a closed channel). A
  future converter is covered the day it is added (`rules/converter_log.rs`).
- **`dir-size`** (#1367) — CLAUDE.md's "aim for ≲10 source files per module
  directory", gated at **12**: a directory under `src/` with more than twelve
  source files directly in it fails, so ≲10 stays the aim and the gate leaves two
  of room. A source file is a `.rs` the size gate does not count as a test
  (`size::is_test_file`: under `tests/` or `fixtures/`, `tests.rs`, `*_test.rs`,
  `*_tests.rs`, `test_*`), the directory's `mod.rs` / `main.rs` / `lib.rs` aside.
  The fix is to cluster by responsibility into subfolders; a directory of
  first-class components clusters by Unity's *Add Component* categories
  (`audio/`, `nav/`, `physics/`, `rendering/`, `ui/layout/`), the same in
  `components/`, `scene/authoring/` and the inspector. The one exception is a flat
  registry, one file per public unit, which says so on a plain `//` comment line
  anywhere in its owner file (`mod.rs`, or the `<dir>.rs` beside it):
  `// lint: allow(dir-size) <why>`; a doc comment (`///`, `//!`) quoting that
  syntax is not an allow. `src/api/` carries it (one file per Lua namespace). The
  rule reads that allow itself, so one left on a directory back under the cap is a
  finding too (`rules/dir_size.rs`). A directory with no owner file (`src/bin/`,
  one binary per file) has nowhere to write the allow, so it takes no exception
  and is reported as itself (`src/bin/:1`). **Where it misfires:** it counts files,
  not responsibilities, so a directory of twelve unrelated files passes and a
  thirteenth file that truly belongs beside the others fails; the fix is still a
  subfolder unless the directory is a flat registry.
- **`entity-arg`** (#1383) — a script passes `nil` for "no entity" (a failed
  `Scene.FindEntityByName`). Bindings that took `id: u32` threw `error converting
  Lua nil to u32` where their neighbours answered the missing-entity default, and
  the reads that took `Option<u32>` each decided what `nil` meant by hand.
  `api::entity_arg::EntityArg` decides it once: `read()` names no entity (id 0,
  which the allocator never hands out), `live(namespace, verb)` is the write's
  error, `get()` the `Option` where `nil` means "none" on purpose. In `src/api/`, a
  closure whose first parameter is the Lua state (`_`, `lua`, `_: &Lua`) and whose
  arguments name an entity typed `u32` or `Option<u32>` fails. The rule reads one
  line and goes by the parameter's name (`rules/entity_arg.rs`): an id under
  another name, in a hand-parsed `MultiValue`, in a closure whose arguments are a
  type alias (`|_, (id, x): Args|`), or after a Lua state spelled `lua: &mlua::Lua`
  gets past it; so does any binding outside `src/api/`.
- **`full-literal`** (#976) — a test builds a first-class component (a `…Component`
  type `Entity` names, with three or more fields) through a constructor or a
  `..rest`, never a struct expression naming every field: such a literal breaks the
  day the component gains a field. When every field is the point, say so:
  `// lint: allow(full-literal) <why>` (`rules/full_literal.rs`).
- **`fullscreen-triangle`** (#1370) — the fullscreen triangle's vertex maths is
  written once, in `engine/shaders/fullscreen.wgsl`. Any other `.wgsl` under
  `engine/` whose code spells `(i << 1u) & 2u` is a finding (comments never count);
  `#import fullscreen` and call `fullscreen::triangle`, `triangle_down` or `uv`.
  The rule reads the shaders itself, since `Scan` holds only Rust, and applies the
  inline allow to them (a reasonless one is reported under `allow`). WGSL inside
  Rust strings is out of reach: `shadergen`'s postfx scaffold imports the module
  like any shader, and a test fixture that must stand alone (`tests/gpu/`) is not
  scanned. A shader that must stand alone says why with
  `// lint: allow(fullscreen-triangle) <why>`.
- **`ignored`** — what you push must be what CI builds. A scanned `.rs` file that
  `git check-ignore --no-index` matches fails, tracked or not, because the next
  new file beside it is the one a push silently drops
  (MatthewLacerda2/scorsese#872). It takes **no allow**: anchor the pattern
  (`/out/`, not `out/`) or move the file. A failed `git` call is a finding too.
- **`log-tag`** (#1139) — engine warnings reach the console and the session and
  MCP replies (`src/logging`), so a reader must see where each came from. In
  `src/`, a warning or error call whose message literal does not open with
  `[Module]` (`[Asset] …`, `[Render] …`) fails. The calls read are `log::warn!` and
  `log::error!` (a space before the parenthesis included), the bare `warn!` /
  `error!` of a file that has a `use log::…`, and `log!` whose level is
  `Level::Warn` or `Level::Error`; an optional `target: "…",` comes first, the
  arguments may spread over the next few lines, and the message may be a raw
  string. A message that is not a literal takes `// lint: allow(log-tag) <why>`
  (`rules/log_tag.rs`).
- **`lua-path-slash`** (#1390) — scene asset paths are `/` on every OS, so a path
  an API function returns to a script must be too: a bare `to_string_lossy()` keeps
  the `\` on Windows and the scene a script writes it into stops resolving on macOS
  and Linux. A non-test `.to_string_lossy()` or `.display()` in `src/api/` or
  `src/dev/lua_surface/` (the two directories that build Lua values) is a finding;
  spell it `crate::core::paths::to_slash`. The rule is lexical: it cannot see a
  `PathBuf` handed to Lua directly (mlua keeps the `\`), and it skips `.to_str()`
  because `mlua::String::to_str` is far commoner there. It does flag a call on an
  `mlua::String` or a literal; that line says so with
  `// lint: allow(lua-path-slash) <why>`.
- **`markers`** — nothing in rusty is temporary. A marker word (`TODO`, `FIXME`,
  `XXX`, `HACK`) standing alone in a scanned `.rs` file fails, comment or not;
  part of a longer identifier (`TODOS`, `MY_HACK`) does not. File an issue instead.
- **`named-enum`** — a script-facing enum's names are written once, by
  `core::named_enum!` (#1329), which writes `ALL`, `name()`, a case-insensitive
  `parse()` (aliases too), `names()` for an error's list and `Display`, and fails
  the build when two spellings match ignoring case. A hand-written table is the
  finding: a file of `src/` that has, for one type `T`, both a parse
  (`fn parse(_: &str) -> Option<Self | T>` in an `impl T`, or a free
  `fn parse_*(_: &str) -> Option<T>`) and a name (`fn name` / `fn as_str` on
  `self` returning `&'static str` in an `impl T`, or a free
  `fn *_name(_: T) -> &'static str`). The two may sit in different `impl T` blocks,
  and an `impl` may be indented or carry a comment after its `{`. It anchors on
  the parse. A `const` / `static` table of `(T, &str)` pairs (`[(T, &str); N]`,
  `&[(T, &'static str)]`) is the same table read both ways, and a finding at its
  declaration; a type `named_enum!` cannot declare (an external crate's enum, such
  as winit's `KeyCode`) takes the allow. A parse that does more than read one name per variant (separators
  ignored, a name built from parts, a `Result` with its own listing) is left alone
  or takes `// lint: allow(named-enum) <why>`. Test code is not scanned.
- **`prefix-group`** — CLAUDE.md's "group by subfolder, not by filename prefix".
  Three or more sibling `.rs` files whose names start with the same word (up to the
  first `_` or `-`) fail. Files count by stem, so `x.rs` and `x_tests.rs` are one,
  and a bare `nav.rs` counts with the `nav_*` files beside it; `mod.rs`, `main.rs`
  and `lib.rs` never count. The finding anchors on the first `mod` line that
  declares the group, which is where its allow goes. The flat registry CLAUDE.md
  exempts (`src/api/`, one file per Lua namespace) is listed in `rules/prefix.rs`
  with the reason, and the exemption covers that directory only, not its
  subfolders.
- **`sanitize`** — every authoring setter refuses a non-finite float (#1361).
  A `pub fn set_*` under `src/scene/authoring/` (test code aside) that takes an
  `f32`, `Vec2`, `Vec3`, `Vec4` or `Quat` fails unless each such parameter is
  passed whole to a helper `scene::authoring::sanitize` defines (`finite`,
  `finite_or`, `at_least`, `non_negative`, `clamped`, `unit`, `unit_color`,
  `finite2`/`3`/`4` and their `_or` forms; the rule reads the list from that file)
  or to another `set_*` of the same file, which it checks in turn. `finite(v.x)`
  sanitizes one component, not `v`, and does not count. The finding anchors on the
  `fn` line; a value refused another way (a joint axis through `try_normalize`)
  says so with `// lint: allow(sanitize) <why>` above it.
- **`skip`** — an ignored test reads as passing to anyone who reads only the
  summary. `#[ignore]` and `#[cfg_attr(…, ignore)]` fail unless an allow says why:
  `// lint: allow(skip) a timing measurement, run by hand` right above it.
- **`slash-join`** (#1332, #1371) — `Path::join("a/b")` keeps the `/`, so on
  Windows it builds `dir\a/b` where joining one part at a time builds `dir\a\b`:
  equal as `Path`s, unequal as strings, and a test comparing strings fails on
  Windows only. The engine keeps one spelling everywhere, production and tests
  alike: in `src/` and `tests/`, a `.join(…)`, `.push(…)`, `Path::new(…)` or
  `PathBuf::from(…)` whose whole argument is a string literal holding a `/` (raw
  or not, wrapped by rustfmt or not), a `format!` whose template holds one, or a
  `/`-holding `&str` constant the scan defines (matched by name, aliases followed)
  is a finding: a path *made* from a `/` path turns mixed the moment something is
  joined onto it. A literal of only `/`s and spaces (`parts.join(" / ")`) is a
  slice's separator, and one starting with `/` is an absolute Unix path (a socket,
  a test's out-of-tree root); neither is flagged. Join one part per call, or use
  `core::paths::native("a/b")` (`crate::core::paths::native` in the library,
  `rusty::core::paths::native` in `tests/`; `native_string` for the written
  string), which turns a `/`-spelled path or constant such as `IMPORT_CACHE_DIR`
  into the platform's. The mix runs the other way too (#1428): a `format!`
  whose template puts a `/` after a positional `{}` filled from a native path's
  `.to_string_lossy()` or `.display()` builds `dir\a/b` on Windows, which a
  baked material once stored in its scene; spell the path with
  `core::paths::to_slash(path)`. The match is coarse: any `{}/` in the template
  plus any `.to_string_lossy()` / `.display()` argument flags, so a ratio such as
  `"{}/{} in {}"` beside a displayed path misfires; it misses `{0}/`, a named
  placeholder (`{DIR}/`), `.to_str()`, `write!` / `println!`, and a native path
  stringified into a variable first. The rule reads text, not types, so it **misfires** on a
  `.push` of a `/` literal (or of a space-free `/` `format!`) onto a list of
  strings, on a slice join whose separator holds a `/` beside other text, and on a
  constant matched by bare name: a `/` constant in one module flags a same-named
  plain one joined in another. It is **blind** to a `/` path held in a variable or
  handed to a helper that joins it; a constant built by a macro, wrapped by rustfmt
  onto the line after `const X: &str =`, typed `&'static str` or declared
  `static`; and a bare `"/a/b"` expectation (test code is full of `/`-led JSON
  pointers): those use the same helper. A path that must keep its `/`, or a
  misfire, says why with `// lint: allow(slash-join) <why>`.
- **`sorted`** (#911) — two branches appending to the end of one list always
  collide on its last line; entries inserted in order land on different lines. A
  list whose order means nothing carries `// sorted: <why order is free>` (Markdown:
  `<!-- sorted: <why> -->`) on the line above it, and fails when an entry sorts
  before the one above it. The list ends at the first blank line or the first line
  indented less than its first entry; an entry's key is the rest of its line from
  the first word that is not a Rust keyword (`pub`, `crate`, `mod`, …), compared
  ignoring case, so `pub(crate) mod a;` sorts as `a;`. Deeper lines continue an
  entry; comment and attribute lines and a Markdown table's header (right under the
  marker) are skipped. A marker with no reason, one wrapped over two lines, or one
  that governs no entries (a blank line under it, or a first entry indented deeper
  than it) is itself a finding. Rust files and the tracked Markdown under `docs/`,
  `.claude/skills/` and the repo root are scanned. A list
  whose order matters (`src/app/registry.rs`'s `build()`, the `Entity` fields,
  `ComponentKind`, `docs/api/index.md`'s namespaces) gets a comment saying why instead of a marker
  (`rules/sorted.rs`).
- **Test code**, for every rule in this list that says it and for the layering
  lint, is one definition (`rules/test_code.rs` over `layers/gate.rs`, #1068,
  #1122): all of `tests/`, every `tests.rs` / `*_tests.rs`, every file only a
  test-gated `mod x;` declares (with or without `#[path]`, and every file it declares
  in turn, #1120), and the item under each `#[cfg(test)]` or
  `#[cfg(all(…, test, …))]` (`test` anywhere among `all`'s arguments, the attribute
  wrapped or not). The rules scan only `src/` and `tests/` for it. `any(test, …)`
  is not test-only (it also compiles outside tests), so it stays production code,
  and its imports are checked against the layer table. A gate covers its own item
  and no more (#1113): a `mod tests { … }` or a `fn` to its closing brace, however
  its signature wraps, a `use` to its `;`, a gated field, variant or arm to its `,`
  (or to the line before its parent's `}`); production code below a gated `use`
  stays production code. Brackets and `//` inside a string or char literal don't
  count, a string that spans lines (a multi-line raw string) included; a `/* */`
  comment (nested ones too) is dropped, so neither its brackets nor its `//` count.
  Every text-scanning rule reads code through the one stripper, `tools/lint/src/code.rs` (#1199). A
  gated field or `let` whose generic type wraps over lines can end early, at a `,`
  inside its `<…>`. Clippy's test exemption (`allow-unwrap-in-tests`, set in
  `clippy.toml`) is Clippy's own and classifies its own way.
- **`test-inner-cfg`, `test-user-workspace`, `test-gpu-entry`** (#980) — three
  `docs/testing.md` locks on test code; comment lines never count. A `tests/` file never gates itself with an inner `#![cfg(…)]` (gate
  its `mod` line); test code never calls `in_user_workspace` or `scenario::run(`
  (use `scenario::run_isolated`); and test code reaches a GPU only through
  `render::test_gpu::headless_or_skip`, never `new_headless` directly
  (`rules/test_shape.rs`).
- **`test-temp-dir`** — a path under `std::env::temp_dir()` is shared by every test
  process, every worktree and (under `cargo test` and cargo-mutants) every test
  thread, so a fixed or process-keyed name races. Test code (the *Test code* bullet)
  calls `crate::test_temp::dir()` (in-crate) or `crate::temp::dir()`
  (integration) instead: both key on the process *and* the running test (#709,
  #999). Comment lines never count. The
  helper itself and Unix socket paths (which must stay short) carry an inline
  allow. `tools/` is not scanned: the lint crate's own tests are not the engine's.
- **The ≲10-files-per-directory signal** prints after the findings: every scanned
  directory holding more than ten source files as `dir-size` counts them, most
  crowded first, leaving out what is past that rule's cap under `src/` (its
  finding or allow). It never fails the gate; it tells you where to cluster by
  responsibility when you next work in that directory, before `dir-size` makes you.
- **`unwrap-deny`** — the unwrap ban covers exactly the layer table's sim rows
  (#962). Each sim row's `src/<module>/mod.rs` must carry
  `#![deny(clippy::unwrap_used)]`, and no file under a sim module may `allow`,
  `expect` or `warn` `clippy::unwrap_used` (or the `clippy::restriction` group).
  A new sim row is covered the day it is added. Platform files are not scanned.
- **`world-space`** (#1170, #1196) — fifteen bugs in two days came from systems
  working out where an entity is in the world and getting a hierarchy case wrong: a
  mirror, a zero scale, a turned child under a non-uniform parent. `ecs::World`'s
  world API (`local_to_world`, `world_pose`, `lossy_scale`, `inverse_transform_frame`, …) is
  the one answer, so a code line of `src/` outside `src/ecs/world_space/` and
  `components::transform` fails when it decomposes a matrix
  (`to_scale_rotation_translation`), names `compute_world_matrix`,
  `parent_world_matrix` or a call `world_matrix(` (a variable of that name passes),
  or has `ancestors(` with a Transform read (`.transform(`, `to_matrix(`, `.pose()`)
  on it or the five lines after. A walk for membership or depth reads no Transform
  and passes. Test code is not scanned (a test may decompose a result to check it),
  which is why the FBX basis tests and the synthetic hitbox rig need no allow; code
  that converts file data rather than the scene hierarchy carries an inline allow.
  The rule is lexical (`rules/world_space.rs`), and its limits are known. It
  **misses** a parent walk not spelled `ancestors(`: `successors(..parent_id)`, a
  `while let Some(p) = parent_id` loop, a chain of `local_and_parent(` calls, and a
  fold whose Transform read sits more than five lines below its walk. It **falsely
  flags** a membership or depth walk that happens to have an unrelated Transform
  read within five lines; that line takes `// lint: allow(world-space) <why>`.

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
  test-only module files) is exempt, by the same definition the rules that exempt test
code use (*Test code*, below). The direction and determinism
  guards are the two that scan test code: their arrow and clock ban hold in tests too.
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
  `Cargo.toml` (`getrandom`: a feature pin, #552). `env_logger` left the list
  when `src/logging` took it over (#1139).
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
the external crates, which the table does not model. Unlike the layering lint, it scans test code too
(#1136): the arrow is the same everywhere, and a sim module's tests stay runnable
without a GPU or a window only while they never reach for the renderer. (`wgpu` is
a plain dependency of the one crate, so it is linked either way; what the guard keeps
is the sim's tests independent of the display layer.)
Plain data the renderer and the sim share lives sim-side: the mesh `Vertex` and the
primitive builders in `components::mesh`, `Camera` and `Decal` in `scene`,
`QualityPreset` in `core::settings::quality`; the renderer keeps only the GPU half (e.g.
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
  (→ `NavMeshAgent`), `post_processing api` (→ `Graphics`).

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
