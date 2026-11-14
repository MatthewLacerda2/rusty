# CLAUDE.md — the workspace's crates

This file loads when you work under `crates/`. The engine is a cargo workspace being
split **one crate per top-level module**: `src/<module>/` becomes `rusty-<module>`
under `crates/<module>/` (`rusty_<module>` in code), one module (or one tight group)
per pull request, bottom-up through `tools/lint/src/layers/table.rs`. The tree is
half moved for many PRs, and everything must work that way. This is the recipe.

## How code names a moved module
- **The root package re-exports each moved crate under its old name**
  (`pub use rusty_core as core;` in `src/lib.rs`), so every `crate::core::…` in
  `src/` and `rusty::core::…` in `tests/` keeps compiling untouched. Don't rewrite
  those paths: they change when their own module moves.
- **Inside a moved crate**, its own items are `crate::…` and a sibling crate is
  `rusty_<module>::…` directly. No alias back to the old names.
- A macro another module calls is `#[macro_export]`, written with `$crate::…`
  (`rusty_core::named_enum!`, reached as `crate::core::named_enum!` from the root).

## Moving a module, step by step
`make move-crate MODULE=<m>` makes steps 1–4 and prints, as *by hand*, what a text
edit cannot settle (a `crate::<x>` naming a module the row does not list, a
`crate::{…}` group, `CARGO_MANIFEST_DIR`, a string path from the checkout's root, an optional dependency no single `dev` /
`editor` feature turns on, a feature other than those two, and every tracked file
outside the crate still naming `src/<m>/`). Read its diff: it is the recipe below, run.
1. `git mv src/<m> crates/<m>/src`, then `git mv crates/<m>/src/mod.rs
   crates/<m>/src/lib.rs`. Its `#![deny(clippy::unwrap_used)]` (a sim row) or its
   clock opt-out (a platform row) stays at the top of `lib.rs`. The module's saved
   proptest seeds move too, `proptest-regressions/<m>/` →
   `crates/<m>/proptest-regressions/`: proptest reads them beside the folder of the
   nearest `lib.rs`, now the crate's, and left at the root they never replay (the
   `orphan-seeds` lint fails on one).
2. Inside the crate: `crate::<dep>::` → `rusty_<dep>::` for each dep in its table
   row (a macro's `$crate::<dep>::` → `::rusty_<dep>::`). An import binding the
   module's name is settled in its plain forms only: `use crate::<dep>;` and a
   one-line, flat `{self, …}` with `self` first have the file's `<dep>::` paths
   rewritten and the import (with its attributes) or its `self` dropped; a re-export
   keeps its name, `pub use rusty_<dep> as <dep>;` or `{self as <dep>, …}`; `use
   crate::<dep> as x;` becomes `use rusty_<dep> as x;`. Any other form a `self` sits
   in (not first, beside a nested group, a comment in the group, `{self,}`, over
   several lines) is listed, never guessed (`.github/scripts/crate_imports.py`). Then
   `crate::<m>::` → `crate::` and `pub(in crate::<m>)` → `pub(crate)` (siblings
   first, so a submodule named like a module, `crate::render::scene::`, stays the crate's own). A module the row does
   not list is left alone and listed: it is a unit test reaching up (*What the
   compiler will name*). Headers that name a path (`//! src/<m>/x.rs — …`) become
   `crates/<m>/src/x.rs` (`make rules`, `self-path`), and `lib.rs`'s, which as a
   `mod.rs` may have named its directory (`//! src/<m> — …`), names `lib.rs`. A relative `include_str!` /
   `include_bytes!` is re-aimed from the file's new folder, in the crate and in any
   file outside whose include names a file of the crate.
3. `crates/<m>/Cargo.toml`: `version`, `edition` and `publish` `.workspace = true`;
   `[lints] workspace = true` (`make determinism` fails a member without it: the
   clock ban and every deny live there); every dependency `{ workspace = true }`,
   the external crates exactly its table row's plus the common ones it names, its
   sibling crates exactly its row's `deps`. A crate only its tests name is a
   `[dev-dependencies]` entry; one the root has under a `[target.…]` table keeps
   that table. A new external version goes in the
   root's `[workspace.dependencies]`, never in a member.
4. The root `Cargo.toml`: the crate in `[workspace.dependencies]` and the root's
   `[dependencies]`; `src/lib.rs`: drop `pub mod <m>;`, add the re-export. A
   `pub mod` gated by a feature stays gated (*Features*).
5. Every tracked file outside the crate that names `src/<m>/` (the tool lists them):
   a file path (a fixture, a mutation glob, a doc) becomes `crates/<m>/src/…`; a
   module path the lint tools read (the layer table, `externs/policy.rs`) stays.
6. `cargo fmt --all` (the rewritten imports re-sort), then
   `cargo check --workspace --all-targets` with `--features dev` and with
   `--no-default-features`, then fix what the compiler names (below), then
   `make gates`.

## What the compiler will name
- **`pub(crate)` reached from another module** becomes `pub`: it is now that
  crate's API, so say so in the PR. An item only tests use elsewhere is a *test hook*.
- **Test hooks.** A `#[cfg(test)]` item is invisible outside its crate, so one that
  another crate's tests call is compiled in every build, `#[doc(hidden)] pub`, with a
  line saying it is a test hook (`core::project::pin_engine`, `core::hulls::forget`,
  `core::wake::rings`). One that needs a crate the hook's own crate has only as a
  dev-dependency (a proptest strategy) is a `#[doc(hidden)] #[macro_export]` macro,
  expanded in the caller's test against the caller's dev-dependency
  (`rusty_ecs::edit_ops!`). Instrumentation kept under `#[cfg(test)]` (a counter a
  test reads) is compiled in too, off until a test arms it (`ecs::world::walks`), or
  off unless the value was built counting (`scene::lighting::lightmap::Rays::counted`). A
  whole file that is test fixture (`ecs::changes::ops`) goes in `.cargo/mutants.toml`'s
  `exclude_globs`, so mutation never asks a test of it.
  Never a `test-support` feature: a dev-dependency feature the normal build lacks
  rebuilds the crate and everything above it for `cargo test` (`make feature-split`).
- **Shared test helpers** live in `rusty-test-support` (`crates/test-support/`),
  only ever a dev-dependency: the per-test temp dir (`temp::dir`), the rebake
  contract (`rebakes::follows_rebakes`), the embedded model files (`models`,
  `models::anim`, `models::quad_file` on disk), a sky panorama on disk
  (`panorama::fixture`), `.cube` lookups as text (`luts`) and the warnings a test logged
  (`logs::init`, `logs::take`, `logs::take_leveled` with each record's level:
  `rusty-logging` needs `script`, so it sits above most members), and a path in the temp dir spelled as a
  script passes it (`temp::lua_path`). A crate's tests call it directly; the root
  keeps `crate::test_temp` as a re-export. A fixture that is plain data moves here; one built on a
  crate's types is a test hook instead.
- **An inherent `impl` a module above writes for a moved crate's type** no longer
  builds (E0116): a method on the type's own data moves into the crate beside it
  (`scene::nav_settings::NavMeshSettings::define_area`), a helper only the module
  above needs becomes its free function (`navigation::bounds`' `grown`).
- **A unit test that reaches into another module** (its items or its private ones)
  moves to that module's tests, or to `tests/`. One reaching *up* (a module above
  the crate, which no crate may depend on) splits: the crate keeps its own half,
  and the half checking the module above moves to that module's tests, through
  the crate's public API (an import check that bakes moves to `scene`'s tests).
  One naming a module *below* that the row does not list (a test baking with
  `procgen`) takes `rusty-<x>` as a `[dev-dependencies]` entry, by hand (the tool
  lists it), and names it `rusty_<x>::`: the layer table does not hold test code.
  Forward the crate's `dev` / `editor` features to it as to any sibling (*Features*).
  *Below* means an earlier row. `make layers` flags a member's dev-dependency on
  itself or on a module at or above its row.
- **Two modules that name each other are one crate**, the pair its submodules under
  a new module name (`script` holds `scripting` and `api`): rename them into
  `src/<new>/` inside the root package first (`crate::api::` → `crate::script::api::`
  in `src/`, `rusty::api::` → `rusty::script::api::` in `tests/`), give the table one row for both, then `make move-crate` the
  new module as any other. Inside the crate the halves stay `crate::api::…` and
  `crate::scripting::…`.
- **`env!("CARGO_MANIFEST_DIR")`** names the crate's folder, not the checkout, and
  so does the working directory a crate's tests run in: use
  `rusty_core::project::checkout()` for a path in the checkout (`engine/`, a fixture).
- **A crate's warnings reach the console** by their log target: `logging` keeps
  `rusty::…` and every `rusty_<module>::…`, so a moved crate needs nothing for it.
- **A build script's `cargo:rustc-env`** reaches only its own package, so a build
  script moves with the code that reads its variables (`crates/core/build.rs`).

## Features
- A crate holding `cfg(feature = "dev")` or `"editor"` code declares that feature
  itself, and the root's feature turns it on directly (`dev = ["rusty-core/dev", …]`).
  A crate with none declares none, even beside a sibling that has one: the root
  already turns that sibling's on.
- **A dependency the root declares `optional` stays optional in the crate**, turned
  on by the crate's feature of the same name (`editor = ["dep:egui"]`). A crate that
  declares a feature forwards it to each sibling that has one (`dev =
  ["rusty-core/dev"]`), a dev-dependency sibling too (`rusty-physics`' `editor`
  turns on `rusty-asset/editor` for its sidecar tests), so its own `cargo test -p`
  build sees the sibling's gated items.
- **A feature-gated module stays gated.** `#[cfg(feature = "dev")] pub mod dev;`
  becomes `#[cfg(feature = "dev")] pub use rusty_dev as dev;`, the root depends on
  `rusty-dev` with `optional = true`, and its `dev` feature turns it on
  (`"dep:rusty-dev"`). A crate depending on such a sibling depends on it optionally
  behind the same feature.
- The player (`--no-default-features`) must still build without egui and without
  `dev`, and must not *link* them: `make player-tree` fails when its resolution
  reaches `egui*`, `rusty-editor`, `rusty-dev` or the providers' HTTP client.

## Profiles
- `[profile.dev.package."*"]` optimises dependencies only, never a member, so a
  moved crate keeps the edit-loop's opt-level 0. A member past ~256 codegen units
  (the root's `codegen-units = 4096` note) gets the same override row.

## The tools already know both places
`tools/lint` (`tools/lint/src/modules.rs`) and the scripts (`.github/scripts/modules.py`)
read a moved file at its **module path**, `src/<m>/…`, so the layer table, the rules,
the determinism and direction gates, `make review-level`'s table and the
`stored-format` markers, coverage isolation and a `SCOPE=diff` mutation run hold a
module in either place. **A path glob does not**: `mutants-sweep.yml`'s
`SWEEP_FILES`, `make mutants-remote SCOPE='src/<m>/**'` and a string path in another
module's code name the file where it lies, so the move rewrites them (step 5);
`mutants-scope.py` refuses any one glob that matches nothing. Keep the two mirrors in
step. A member that is no module of the table (`test-support`) is listed in both
`NOT_MODULES`, which `make scripts` holds equal (`modules.drift`).

## What the PR shows
- The `cargo check` matrix and `make gates` green on its head.
- Each `pub(crate)` made `pub`, each test hook added, each test moved, by name.
- No doc or skill still names a `src/<m>/` path that moved (`make docrefs` checks
  paths in backticks; grep for the rest).
