# CLAUDE.md — the workspace's crates

This file loads when you work under `crates/`. The engine is a cargo workspace of
**one crate per top-level module**: `rusty-<module>` under `crates/<module>/`
(`rusty_<module>` in code), one row each of `tools/lint/src/layers/table.rs`,
bottom-up. The root package holds only the binaries (`src/main.rs`, `src/bin/`), the
integration tests (`tests/`) and `src/lib.rs`'s re-exports. These are the rules a
crate follows, and how to add one.

## How code names a crate
- **The root package re-exports each crate under its module's name**
  (`pub use rusty_core as core;` in `src/lib.rs`), so the binaries and `tests/` name
  one surface, `rusty::core::…`.
- **Inside a crate**, its own items are `crate::…` and a sibling crate is
  `rusty_<module>::…` directly. No alias back to the root's names.
- A macro another crate calls is `#[macro_export]`, written with `$crate::…`
  (`rusty_core::named_enum!`).

## What cargo holds, and what `make layers` adds
A crate can name only what its `Cargo.toml` depends on, and a cycle of
`[dependencies]` does not resolve: cargo holds the edges. `make layers` holds the
layer table **equal** to the manifests (each row's `deps` are its sibling
`[dependencies]`, its `uses` its external ones, plus the common crates in
`tools/lint/src/layers/externs/policy.rs`), and adds what cargo cannot know: the
rows' bottom-up order, the sim mark (a sim crate depends only on sim crates and
never on a display crate, `wgpu`, `egui*`, `rusty-render` or `rusty-editor`, not
even as a dev-dependency), a dev-dependency only on a crate below its row (cargo
accepts a dev-dependency cycle), and no declared edge that live code no longer
names (cargo builds an unused dependency without a word).

## Adding a crate
1. `crates/<m>/src/lib.rs`, with `#![deny(clippy::unwrap_used)]` at its top for a
   sim row, or the clock opt-out for a platform row that reads real time (`make
   determinism` lets only a platform root opt out).
2. `crates/<m>/Cargo.toml`: `version`, `edition` and `publish` `.workspace = true`;
   `[lints] workspace = true` (`make determinism` fails a member without it: the
   clock ban and every deny live there); every dependency `{ workspace = true }`.
   A crate only its tests name is a `[dev-dependencies]` entry; a platform-specific
   one sits under its `[target.…]` table. A new external version goes in the root's
   `[workspace.dependencies]`, never in a member.
3. A row in the layer table, in its bottom-up place, `sim` or `platform`, its
   `deps` and `.uses(&[…])` exactly the manifest's (`make layers` names any
   difference).
4. The root `Cargo.toml`: the crate in `[workspace.dependencies]` and the root's
   `[dependencies]`; `src/lib.rs`: its re-export (gated, when the crate is a
   feature's: *Features*).
5. `cargo check --workspace --all-targets` with `--features dev` and with
   `--no-default-features`, then `make gates`.

A member that is no module of the table (`test-support`) is listed in both
`NOT_MODULES`, `tools/lint/src/modules.rs` and `.github/scripts/modules.py`, which
`make scripts` holds equal (`modules.drift`).

## Splitting a crate
A sub-module leaving an existing crate for a new one, by hand: `git mv`, the
manifest, the row, then the compiler names every path left to fix.
- **Cut where nothing below names the half that moves.** Map who uses each
  sub-module first; what the staying half's live code reaches (a type a field of
  the base holds, the settings it stores) moves *down* into the base before the
  cut, and the half above names it there (`scene::lighting::settings::BakeSettings`).
  A crate *above* the new one whose tests alone reach it takes it as a
  `[dev-dependencies]` entry, never a live edge (`navigation`'s tests and
  `rusty-lighting`); a crate below it cannot, so such a test moves up instead.
- Callers name the new crate directly: the base cannot re-export it (that would be a
  dependency cycle).
- **Inside the new crate**, `crate::<old>::<m>` becomes `crate::<m>`, the old
  crate's own items become `rusty_<old>::…`, and `pub(in crate::<old>)` becomes
  `pub(crate)`. A `use super::*` test reaching a moved item finds it through a
  `#[cfg(test)] use` in its parent; rustdoc does not see that one, so an
  intra-doc link to the item names its full path.
- **What tests reach across the cut** becomes a test hook (a `#[cfg(test)]`
  method the moved tests call turns `#[doc(hidden)] pub`), and an inherent method
  the new crate wrote on a base type becomes its free function (E0116).
- **Paths kept outside the code follow**: `include_str!`'s relative paths (one
  folder shallower or deeper), `.cargo/mutants.toml`'s globs, a `stored-format`
  marker whose directory no longer covers a moved file (the moved file takes its
  own), `layers`' pinned sim set, and prose naming the old module path.

## Crossing a crate boundary
- **An item another crate reaches is `pub`**: it is that crate's API, so a PR making
  one `pub` says so. An item only tests use elsewhere is a *test hook*.
- **Test hooks.** A `#[cfg(test)]` item is invisible outside its crate, so one that
  another crate's tests call is compiled in every build, `#[doc(hidden)] pub`, with a
  line saying it is a test hook (`core::project::pin_engine`, `core::hulls::forget`,
  `core::wake::rings`). One that needs a crate the hook's own crate has only as a
  dev-dependency (a proptest strategy) is a `#[doc(hidden)] #[macro_export]` macro,
  expanded in the caller's test against the caller's dev-dependency
  (`rusty_ecs::edit_ops!`). Instrumentation kept under `#[cfg(test)]` (a counter a
  test reads) is compiled in too, off until a test arms it (`ecs::world::walks`,
  `logging::init_for_tests`; a `cfg!(test)` switch is false for other crates'
  tests, `cfg-test-switch`), or
  off unless the value was built counting (`lighting::lightmap::Rays::counted`). A
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
  script passes it (`temp::lua_path`). A crate's tests call it directly, the
  integration suite through `tests/temp.rs`; a folder spelled from the working
  directory is `temp::relative`. A fixture that is plain data moves here; one built on a
  crate's types is a test hook instead.
- **An inherent `impl` a crate above writes for a sibling's type** does not
  build (E0116): a method on the type's own data moves into the crate beside it
  (`scene::nav_settings::NavMeshSettings::define_area`), a helper only the module
  above needs becomes its free function (`navigation::bounds`' `grown`).
- **A unit test that reaches into another crate's private items** moves to that
  crate's tests, or to `tests/`. One reaching *up* (a crate above, which no crate
  may depend on) splits: the crate keeps its own half, and the half checking the
  crate above moves to that crate's tests, through the lower crate's public API.
  One naming a crate *below* that the row does not list (a test baking with
  `procgen`) takes `rusty-<x>` as a `[dev-dependencies]` entry and names it
  `rusty_<x>::`: the table's `deps` are live code's. Forward the crate's `dev` /
  `editor` features to it as to any sibling (*Features*). *Below* means an earlier
  row; `make layers` flags a dev-dependency on the crate itself or on one at or
  above its row.
- **Two modules that name each other are one crate**, the pair its submodules
  (`script` holds `scripting` and `api`, one row for both). Inside the crate the
  halves are `crate::api::…` and `crate::scripting::…`.
- **`env!("CARGO_MANIFEST_DIR")`** names the crate's folder, not the checkout, and
  so does the working directory a crate's tests run in: use
  `rusty_core::project::checkout()` for a path in the checkout (`engine/`, a fixture).
- **A crate's warnings reach the console** by their log target: `logging` keeps
  `rusty::…` and every `rusty_<module>::…`, so a crate needs nothing for it.
- **A build script's `cargo:rustc-env`** reaches only its own package, so a build
  script moves with the code that reads its variables (`crates/core/build.rs`).

## Features
- **A crate whose tests link `rusty-dev`** (a dependency or dev-dependency) expands
  `rusty_dev::no_provider_keys!();` once in its tests, under `#[cfg(all(test,
  feature = "dev"))]`: no test binary may hold a provider key (`key-guard`).
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
- **A feature's crate is an optional dependency.** `rusty-dev` and `rusty-editor`
  are re-exported gated (`#[cfg(feature = "dev")] pub use rusty_dev as dev;`), the
  root depends on each with `optional = true`, and its feature turns it on
  (`"dep:rusty-dev"`). A crate depending on such a sibling depends on it optionally
  behind the same feature.
- The player (`--no-default-features`) must still build without egui and without
  `dev`, and must not *link* them: `make player-tree` fails when its resolution
  reaches `egui*`, `rusty-editor`, `rusty-dev` or the providers' HTTP client.

## Profiles
- `[profile.dev.package."*"]` optimises dependencies only, never a member, so a
  crate keeps the edit-loop's opt-level 0.

## Module paths in the tools
`tools/lint` (`tools/lint/src/modules.rs`) and the scripts (`.github/scripts/modules.py`)
read a crate's file at its **module path**, `src/<m>/…` (`lib.rs` read as
`mod.rs`): the one spelling the rules' scopes, the layer table's sim and platform
sets, the determinism gate, `make review-level`'s table, the `stored-format`
markers, coverage isolation and a `SCOPE=diff` mutation run share. Write a scope
there as a module path; a finding still names the real `crates/<m>/src/…` path.
**A path glob does not** go through it: `mutants-sweep.yml`'s `SWEEP_FILES`, `make
mutants-remote SCOPE='crates/<m>/src/**'` and a string path in code name the file
where it lies; `mutants-scope.py` refuses any one glob that matches nothing. Keep
the two mirrors in step.

## What a PR adding or splitting a crate shows
- The `cargo check` matrix and `make gates` green on its head.
- Each item made `pub` for another crate, each test hook added, each test moved,
  by name.
