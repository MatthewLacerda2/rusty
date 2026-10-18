# CLAUDE.md — the lint tool

This file loads when you work under `tools/lint/`: the binary behind `make size`,
`make rules`, `make layers`, `make determinism`, `make direction`, `make components` and
`make parity`. The root `CLAUDE.md` says what each gate protects and why a convention
a machine can check is a gate; `docs/linting.md` is the reference for every rule. This
file is what a coder changing the tool must keep.

## Held to what it enforces
- **Zero dependencies.** std only: `tools/lint/Cargo.lock` names one package, itself,
  and `tools/lint/src/rules/lockfile_tests.rs` fails on a second (`make test-lint`). A
  dependency is a decision with its own issue, not a drive-by.
- **Clippy-gated like the engine** (#935): `make clippy` and CI run it with
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
  nothing grandfathered (#926). A standing decision that is no single line's exception
  (a component axis waived for good) is a row with its rationale, like `WAIVERS` in
  `tools/lint/src/components/waivers.rs`.
- **A new rule is a module, not a flag.** It goes in `tools/lint/src/rules/<rule>.rs`,
  returns `Finding`s, and is one more call in `rules::run`: no new flag, gate, CI step
  or baseline. Call it **before** the `findings.retain(…)` that applies the allows when
  an inline allow may excuse it, **after** when nothing may: `ignored` runs there
  because its fix is the `.gitignore` pattern, never an allow.
- **Signals print and never fail.** What CLAUDE.md only says to *aim* for (≲10 files
  per directory) prints under `signal (not a failure)` after the findings and leaves
  the exit code alone. It becomes a finding only with the decision that makes it a
  rule.
- **One report shape**: `path:line: [rule] message` on the terminal and in
  `.lint/report.txt`, exit 1 on any finding, so an agent reads exactly what failed.

## The layer table
- **`tools/lint/src/layers/table.rs` is the one place module edges and external-crate
  uses are declared** (#724, #938): one row per `src/<module>`, bottom-up, `sim` or
  platform, its crates in `.uses(&[…])`. What belongs to no single row (crates any
  module may name, crates confined to a subtree, dependencies no module names, and
  exceptions with their open issue) is in `tools/lint/src/layers/externs/policy.rs`.
- **Derive, don't copy.** The direction and determinism gates read the sim set from
  the table (`layers::sim_dirs`, `layers::platform_roots`); a new check that asks
  "which modules are sim?" reads it the same way.
- The table is exact, not a ceiling: an entry nothing uses fails as stale.

## Docs
- **`docs/linting.md` lists every gate**: one row per check in *What's enforced*, and
  a section per rule. A new or changed rule updates it in the same pull request.
