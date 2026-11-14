# CLAUDE.md — the lint tool

This file loads when you work under `tools/lint/`: the binary behind `make size`,
`make rules`, `make layers`, `make determinism`, `make components` and
`make parity`, and the `make dead-pub` signal. The root `CLAUDE.md` says what each gate protects and why a convention
a machine can check is a gate; `docs/linting.md` is the reference for every rule. This
file is what a coder changing the tool must keep.

## Held to what it enforces
- **Zero dependencies.** std only: `tools/lint/Cargo.lock` names one package, itself,
  and `tools/lint/src/rules/lockfile_tests.rs` fails on a second (`make test-lint`). A
  dependency is a decision with its own issue, not a drive-by.
- **Clippy-gated like the engine**: `make clippy` and CI run it with
  `-D warnings`, and its own `[lints]` carries the nothing-temporary denies. The size
  gate scans it too (`tools/` is a scan root).
- **A standalone crate** with its own `[workspace]`: never fold it into the engine.

## Writing a rule
- **Every rule has a must-flag and a must-pass test**: a case it has to report and a
  case it has to leave alone, in its tests.
- **Exceptions are inline, with a reason.** Code that breaks a rule on purpose says
  `// lint: allow(<rule>) <why>` on the flagged line or in the comment and attribute
  lines right above it (`tools/lint/src/rules/finding.rs`). An allow with no reason is
  itself a finding (`[allow]`). No baseline file, no exception list in a config,
  nothing grandfathered. A standing decision that is no single line's exception
  (a component axis waived for good) is a row with its rationale, like `WAIVERS` in
  `tools/lint/src/components/waivers.rs`.
- **A new rule is a module, not a flag.** It goes in `tools/lint/src/rules/<rule>.rs`
  with `pub fn run(&Scan) -> Vec<Finding>`, mounts its own `<rule>_tests.rs` with
  `#[path]`, and takes one line in `rules/mod.rs`'s sorted `mod` list and one row in
  the sorted `RULES` table, so parallel rule branches touch different lines:
  no new flag, gate, CI step or baseline. Its row says `Allow::Inline` when an inline
  allow may excuse it, `Allow::Never` when nothing may: `ignored` (its fix is the
  `.gitignore` pattern), `orphan-seeds` (its fix is moving the file) and `key-guard`
  (its finding sits on a manifest, where no inline allow can go). `Allow::Own` is for
  a rule that reads its own allows, so the shared filter is skipped: `api-parity`, whose allow sits on the op
  rather than on the finding's cause, and must be reported when it goes stale, and
  `dir-size`, whose allow sits anywhere in the directory's owner file.
- **What holds Markdown rides the size gate, not `--rules`**: `agent_docs/` (every
  `CLAUDE.md` and skill file) runs in `make size`, which the commit hook and CI's
  Markdown-only runs reach and `--rules` does not. It still reports `Finding`s and
  reads the shared allow, whose Markdown form is `<!-- lint: allow(<rule>) <why> -->`.
- **Signals print and never fail.** What CLAUDE.md only says to *aim* for (≲10 files
  per directory) prints under `signal (not a failure)` after the findings and leaves
  the exit code alone. It becomes a finding only with the decision that makes it a
  rule: `dir-size` fails past 12, and the signal still prints the 11s and 12s.
- **One report shape**: `path:line: [rule] message` on the terminal and in
  `.lint/report.txt`, exit 1 on any finding, so an agent reads exactly what failed.

## The layer table
- **`tools/lint/src/layers/table.rs` maps the workspace's crates**: one row per member
  `crates/<module>`, bottom-up, `sim` or platform, its sibling deps and its external
  crates in `.uses(&[…])`, held **equal** to each member's `Cargo.toml`. What belongs
  to no single row (crates any module may name, crates confined to a subtree,
  dependencies no module names, the display crates no sim member may take) is in
  `tools/lint/src/layers/externs/policy.rs`.
- **Check only what cargo cannot.** A crate cannot name what it does not depend on,
  so a pass that would flag an undeclared edge in code can never fail: the layer
  lint reads the source only for unused edges and confined crates.
- **Derive, don't copy.** The determinism gate and the rules read the sim set from
  the table (`layers::sim_dirs`, `layers::platform_roots`); a new check that asks
  "which modules are sim?" reads it the same way.
- The table is exact, not a ceiling: an entry nothing uses fails as stale.
- **A pass that scopes by module reads a file at its module path**, `src/<m>/…` for
  `crates/<m>/src/…` (`modules::module_path`; a folder from `modules::dir`), never a
  bare `starts_with("crates/…")`, and reports the real path.
  `.github/scripts/modules.py` is its mirror: change both together.

## Docs
- **`docs/linting.md` lists every gate**: one row per check in *What's enforced*, and
  a section per rule. A new or changed rule updates it in the same pull request.
