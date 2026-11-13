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
edit cannot settle (a `crate::{…}` group, `CARGO_MANIFEST_DIR`, a feature gating an
optional dependency). Read its diff: it is the recipe below, run.
1. `git mv src/<m> crates/<m>/src`, then `git mv crates/<m>/src/mod.rs
   crates/<m>/src/lib.rs`. Its `#![deny(clippy::unwrap_used)]` (a sim row) or its
   clock opt-out (a platform row) stays at the top of `lib.rs`.
2. Inside the crate: `crate::<m>::` → `crate::`, and `crate::<dep>::` →
   `rusty_<dep>::` for each dep in its table row. Headers that name a path
   (`//! src/<m>/x.rs — …`) become `crates/<m>/src/x.rs` (`make rules`, `self-path`).
3. `crates/<m>/Cargo.toml`: `version`, `edition` and `publish` `.workspace = true`;
   `[lints] workspace = true` (`make determinism` fails a member without it: the
   clock ban and every deny live there); every dependency `{ workspace = true }`,
   the external crates exactly its table row's plus the common ones it names, its
   sibling crates exactly its row's `deps`. A new external version goes in the
   root's `[workspace.dependencies]`, never in a member.
4. The root `Cargo.toml`: the crate in `[workspace.dependencies]` and the root's
   `[dependencies]`; `src/lib.rs`: drop `pub mod <m>;`, add the re-export.
5. `cargo check --workspace --all-targets` with `--features dev` and with
   `--no-default-features`, then fix what the compiler names (below), then
   `make gates`.

## What the compiler will name
- **`pub(crate)` reached from another module** becomes `pub`: it is now that
  crate's API, so say so in the PR. An item only tests use elsewhere is a *test hook*.
- **Test hooks.** A `#[cfg(test)]` item is invisible outside its crate, so one that
  another crate's tests call is compiled in every build, `#[doc(hidden)] pub`, with a
  line saying it is a test hook (`core::project::pin_engine`, `core::hulls::forget`).
  Never a `test-support` feature: a dev-dependency feature the normal build lacks
  rebuilds the crate and everything above it for `cargo test` (`make feature-split`).
- **Shared test helpers** live in `rusty-test-support` (`crates/test-support/`),
  only ever a dev-dependency: the per-test temp dir (`temp::dir`) and the rebake
  contract (`rebakes::follows_rebakes`). A crate's tests call it directly; the root
  keeps `crate::test_temp` as a re-export beside its `lua_path`.
- **A unit test that reaches into another module** (its items or its private ones)
  moves to that module's tests, or to `tests/`.
- **`env!("CARGO_MANIFEST_DIR")`** names the crate's folder, not the checkout: use
  `rusty_core::project::checkout()` for a path in the checkout (`engine/`, a fixture).
- **A build script's `cargo:rustc-env`** reaches only its own package, so a build
  script moves with the code that reads its variables (`crates/core/build.rs`).

## Features
- A crate holding `cfg(feature = "dev")` or `"editor"` code declares that feature
  itself (`dev = []`, or the `dep:` it gates), and the root's feature forwards it
  (`dev = ["rusty-core/dev", …]`). A crate with none declares none. The player
  (`--no-default-features`) must still build without egui and without `dev`.

## Profiles
- `[profile.dev.package."*"]` optimises dependencies only, never a member, so a
  moved crate keeps the edit-loop's opt-level 0. A member past ~256 codegen units
  (the root's `codegen-units = 4096` note) gets the same override row.

## The tools already know both places
`tools/lint` (`tools/lint/src/modules.rs`) and the scripts (`.github/scripts/modules.py`)
read a moved file at its **module path**, `src/<m>/…`, so the layer table, the rules,
the determinism and direction gates, `make review-level`'s table and the
`stored-format` markers, coverage isolation and the mutation scopes hold a module in
either place. Keep the two mirrors in step. A member that is no module of the table
(`test-support`) is listed in both `NOT_MODULES`.

## What the PR shows
- The `cargo check` matrix and `make gates` green on its head.
- Each `pub(crate)` made `pub`, each test hook added, each test moved, by name.
- No doc or skill still names a `src/<m>/` path that moved (`make docrefs` checks
  paths in backticks; grep for the rest).
