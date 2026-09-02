---
name: ci-merge
description: Take a finished branch through to merged — the gates, CI, verifying a run happened on the head commit, rebasing, and triaging the mutation and coverage signals. Use when a branch is ready, when a pull request has gone red, when a mutation report needs triage, or when merging anything into `main`.
---

# Getting a branch merged

The steps, and the traps. `CLAUDE.md` carries why these exist; this is how.

**There is no `make` here.** Every gate is a `cargo` line, and the list below is
the list — `docs/linting.md` and `.github/workflows/{ci,lint}.yml` are the two
sources of truth, and they agree.

## Before marking a pull request ready

All of these must be green. They are what CI blocks on, in the order that fails
fastest first:

    cargo fmt --all --check
    cargo fmt --manifest-path tools/lint/Cargo.toml --check
    cargo run --quiet --manifest-path tools/lint/Cargo.toml                  # size gate, full scan
    cargo run --quiet --manifest-path tools/lint/Cargo.toml -- --determinism
    cargo run --quiet --manifest-path tools/lint/Cargo.toml -- --components
    cargo run --quiet --manifest-path tools/lint/Cargo.toml -- --parity
    cargo clippy --all-targets -- -D warnings -D clippy::too_many_lines
    cargo clippy --all-targets --features dev -- -D warnings -D clippy::too_many_lines
    cargo build --locked && cargo build --features dev --locked
    cargo test --locked && cargo test --features dev --locked
    cargo test --manifest-path tools/lint/Cargo.toml --locked
    cargo deny check advisories bans sources licenses

**Both feature sets, always.** Half this crate is behind `dev` — the harness, the
session, the MCP bridge, `Debug.*` — and a default-features-only run compiles none
of it. The two clippy passes and the two test passes are not redundancy.

**Some hard gates hide inside `cargo test`.** `tests/api_doc_drift.rs` and
`tests/callback_doc_drift.rs` fail the build when `docs/scripting-api.md`
disagrees with the live Lua surface *in either direction* — an undocumented
binding and a documented-but-absent one both redden CI. They are dev-only, so
only the `--features dev` run sees them.

**The local hook is not a substitute and may not exist.** `lefthook.yml` runs
`fmt`, the size gate and default-feature clippy only — no build, no tests, no
`--components`, no `--parity`, no dev feature set. And it only runs if somebody
ran `lefthook install` in this clone; `git config core.hooksPath` and
`ls .git/hooks` say whether they did. Assume nothing was caught for you.

The full list is deliberately **not** run before every push. Checkpoint commits
stay cheap.

**Failures land in `.lint/report.txt`** — read it rather than re-running the tool
to see what it said.

*(Gap worth building: nothing wraps this list. It is copied by hand into every
session, it drifts, and no entry point exists that could also refuse to run under a
shared `CARGO_TARGET_DIR`. One `tools/` command that runs the whole list is
`infrastructure`-label work.)*

## The merge, one branch at a time

1. `git fetch origin && git rebase origin/main` in the branch's worktree.
2. Push with `--force-with-lease`.
3. Wait for CI **on the rebased head**.
4. Verify a run actually happened on that head — see the next section.
5. Merge (squash; the house style is a title carrying `(#N)`). Then remove the
   worktree and delete the branch — a stale worktree is ~12–16 GB.

Merging is serialized because rusty is **one compiled crate**: two branches can
each be green alone and break `main` together. A rename, a changed signature, a
moved module — no textual conflict catches any of them, and being a single crate
makes it *more* likely, not less, because everything is in scope of everything.

### The Markdown exception, and the file it does not cover

CI's `changes` filter is `code: - '!**/*.md'`. A pull request touching only
Markdown skips the heavy *steps*, but the gating jobs still **run** and report
`success` — a job whose steps all skip still succeeds. That is deliberate, and the
workflow comments say why: a ruleset reads a *skipped* required check as
unsatisfied, not as a pass, so skipping the job outright would wedge a docs-only
pull request. So a docs-only pull request needs no serialization and merges
freely.

**`docs/scripting-api.md` is not one of those.** Two hard-gate tests parse it, so
a Markdown-only edit to it can break `main` on its own. Same for
`docs/api-faithfulness.md`'s catalog if a branch is relying on it. Treat either as
code.

## Verify the run happened on the head commit

No script does this for you, and — **as `main` is configured today** — nothing on
GitHub does either. Its one ruleset rule is *deletion*: no required status checks,
no merge queue, so GitHub will happily let a red or entirely unbuilt pull request
merge. Confirm before trusting otherwise:

    gh api repos/:owner/:repo/rulesets --jq '.[].id' \
      | xargs -I{} gh api repos/:owner/:repo/rulesets/{} --jq '.name, [.rules[].type]'

Until that says something stricter, the whole check is yours.

`gh pr checks N` is **not** it. It lists the pull request's checks with no commit
column at all, so it cannot answer "did this run build the head I am about to
merge?", and it blends runs — a skipped job sits in the same list as a real one.
Ask about the commit instead:

    SHA=$(gh pr view N --json headRefOid -q .headRefOid)
    gh api "repos/:owner/:repo/commits/$SHA/check-runs" \
      --jq '.check_runs[] | "\(.name)\t\(.status)\t\(.conclusion)"' | sort

A healthy code branch shows: `build-test`, `build-test-cross (macos-latest)`,
`build-test-cross (windows-latest)`, `deny`, `ci-gate` from `ci.yml`; `lint` and
`lint-gate` from `lint.yml`; `changes` **twice**, once per workflow — that
duplicate is normal and is exactly the shape that makes eyeballing a check list
unreliable. `coverage` and `mutants` appear as **skipped** on every pull request
by design (they are the post-merge and nightly runs). Skipped is the honest
answer; never read it as green.

`ci-gate` and `lint-gate` are the two that matter — each collapses its workflow's
gating jobs into one verdict and passes only when every one of them succeeded or
was legitimately skipped. If those two are `success` on the head SHA, the gates
are green on the code being merged.

Three failure shapes to expect:

- **A skipped run reading as green.** See above — `coverage`, `mutants`, and every
  step on a Markdown-only pull request.
- **No run at all, because the pull request was readied moments after a push.**
  The checks are not green, they are absent. `ci.yml` triggers on
  `ready_for_review`, so this usually self-corrects; if it does not, force one with
  an empty commit.
- **No run at all, because the branch conflicts with `main`.** GitHub cannot build
  a merge ref for a conflicted branch, so it creates nothing — no run, no check, no
  error. This reads exactly like a broken workflow file.

**Telling the last two apart**, three lines:

1. **No run at all on a ready pull request → check whether the branch conflicts
   with `main`**, before touching a workflow file.
2. **An invalid workflow produces a run** — a `push`-event `startup_failure`. That
   is how the two are told apart. No run whatsoever means conflict.
3. **The fix is a rebase**, and it is the same rebase step 1 asks for anyway — so
   it costs nothing but doing it now.

**Never hand-roll a "wait for CI" loop that treats zero checks as success.**
Absent and passing are different states; a loop counting non-completed checks
finds zero of each. Require checks to **exist** before calling a run settled.

*(Two gaps, both real: nothing wraps the query above into a one-line verdict, and
nothing runs a queue of finished branches, so an agent holds a worktree open
through every ten-minute run. Both workflows already trigger on `merge_group`, and
each already exposes a single collapsed gate job built for exactly that — the
machinery is written and the ruleset simply does not turn a queue on. Enabling it
is the user's call, not an assumption to work from.)*

## A red ready pull request stays ready

Fixed in the next commit; it does not go back to draft. Draft is for work that is
genuinely unfinished, blocked, or handed over — including the hand-back after ≈3
attempts at the same failure, which does go to draft, with the reason in the
description.

## Rebasing

Expect conflicts wherever every feature appends: `Entity`'s `Option<…Component>`
fields, `ComponentKind` and its hard-coded `ALL` length, `api/mod.rs`'s module
list, `app/registry.rs`'s ordered `register` calls, the Add Component menu,
`docs/scripting-api.md`'s tables, `scripting/callbacks.rs`, and the burn-down
baselines where both sides *removed* lines.

**Two authors both being right is the common case**, and the resolution is usually
to keep both sides, ordered deliberately rather than by merge accident. Two of
those need more than that:

- `ComponentKind::ALL` is `[ComponentKind; 10]`. Keeping both sides' variants
  leaves the length wrong, and neither diff contains the right number.
- `app/registry.rs`'s `build()` is the **only** place per-frame system order is
  defined. Merging it by textual proximity silently reorders the schedule.

Mechanical resolutions (a `mod` list, an import) are fine to do directly. Hand a
rebase back to the branch's author when resolving it needs to know *why* the code
is shaped as it is — a new variant that should join a documented grouping, two
prose paragraphs that need ordering, a signature that has grown a parameter, a
system whose stage placement was argued for.

After a rebase, re-check any claim the branch made **about the base it measured
against** — the "Gates" paragraph in the description, a survivor count, a coverage
number, a byte-identical-bake proof. A measurement taken against an older `main`
is stale, and citing it is worse than not having run it.

## The same-commit obligations

Four things a branch owes in the *same* change, each backed by a gate that will
otherwise fail at merge time:

- **A binding change updates `docs/scripting-api.md`.** Existence parity, both
  directions, dev feature set. Signatures are out of scope.
- **A new lifecycle callback updates the same doc's callback section**, against
  `src/scripting/callbacks.rs`.
- **A new first-class component satisfies all four axes** — the `Entity` field,
  the Add Component entry, an inspector card, an `src/api/<x>.rs` namespace
  registered and documented — or gets a `WAIVERS` row in
  `tools/lint/src/components.rs` with a written rationale. `--components`
  discovers it from `Entity` itself, so it cannot be slipped past.
- **A migrated inspector card keeps routing through `scene::authoring`.**
  `--parity` fails on a direct field write, *and* on a stale baseline line for a
  card that is now routed. Burn baselines down; never add to one.

And one that is not a gate but bites the same way: **a branch adding a new
top-level `src/<dir>/` must add it to the `UNFLOORED` list in `ci.yml`'s coverage
ratchet step.** A missing directory leaks into every floored module's number —
`audio`, `procgen` and `shadergen` each did exactly that.

There is no version constant to bump here — rusty has no format version and no
bake version. What plays that role is the **determinism guard**: a wall-clock read
or an unseeded RNG anywhere in `app`, `scripting`, `physics` or `navigation`
breaks replay for every harness run, and the gate refuses it rather than letting it
land quietly.

## The signals

Mutation and coverage are **signals, never gates**. Neither can fail a build and
**neither holds a merge**. Both run in CI on their own — there is nothing to
remember to launch.

| | per pull request | the backstop |
|---|---|---|
| mutation | `mutants-pr`, `--in-diff`, sticky comment | `mutants`, nightly, full sim, artifact |
| coverage | `coverage-pr`, `diff-cover`, sticky comment | `coverage`, post-merge on `main`, ratchet table |

Three things about their scope, all of which change how a clean report reads:

- **Mutation only ever touches `src/app`, `src/scripting`, `src/physics`,
  `src/navigation`.** A survivor in `render`, `editor`, `api` or `asset` will never
  be reported. Silence there means unmeasured, not clean.
- **The `sim` paths filter can skip both jobs entirely.** If the branch touched no
  sim source, no `tests/`, and neither `Cargo.toml`/`Cargo.lock`/`rust-toolchain.toml`,
  `mutants-pr` and `coverage-pr` do not run. **No comment is not a clean bill** —
  check the job actually ran, on the head SHA, the same way as everything else.
- **The sticky comments delete themselves** when a re-push fixes the finding
  (headers `mutants-diff` and `coverage-diff`). So an absent comment has three
  possible meanings: clean, fixed, or never measured.

### Triaging a survivor

Read the report when it lists survivors in code **this branch wrote**. A report
with nothing in it, or whose survivors sit in untouched code, needs no reading.

Sort by cost:

- **Fix what is cheap** while the code is still in hand.
- **File a real bug** and fix it *after* the current branch merges.
- **File an architectural or foundational crack** — and do not start it without
  the user's judgement.
- Or **exclude it**, with a written reason, **line-qualified** so an unqualified
  entry cannot also swallow a real gap next door.

Stop the queue only for a report saying a **module** has nothing asserting its
mechanism at all. That is one finding about the tests, not a list of survivors.

**Establish equivalence by applying the mutation and running the suite** — never
by reasoning about symmetry. Reasoning has been wrong repeatedly; measurement has
not. Reproduce a CI survivor locally with the same shape the job uses:

    git diff origin/main > pr.diff
    cargo mutants --in-diff pr.diff --file 'src/physics/**/*.rs' -- --features dev

`cargo mutants --list` names the functions and builds nothing — reach for it before
believing a structural explanation of a bad report.

Two shapes worth recognising before triaging:

**A measurement that discards sign cannot test an operation that changes it.**
`Vec3::length`, a distance, a magnitude, a squared term, a count, an absolute
value, a `Quat` dot — every one of them reads like a real assertion and every one
is blind to a `+` → `-`. In a physics and navigation sim that is most of what the
tests assert on.

**A boundary comparison surviving both `<=` and `>=` means neither end is
exercised** — a fixed-timestep accumulator that never lands exactly on a step, a
nav agent exactly on its arrival radius, a contact exactly at the epsilon, a
trigger overlap beginning and ending on the same tick. Each of those is a real gap
until measured otherwise.

Two caveats that are the reason this is not a gate: `--in-diff` line matching can
drift after a rebase, and a mutant that induces a slow loop times out and reports
as a survivor. Neither should redden CI, and neither should stop a merge.

### The coverage floors

`coverage-baseline.txt` holds per-module line floors for `app`, `scripting`,
`physics`, `navigation` and `api`. It is a **ratchet, not a gate**: the job that
reads it runs post-merge on `main`, is `continue-on-error`, and prints a table
flagging a drop without ever exiting non-zero. Raise a floor as coverage improves;
**never lower one silently to make a table green** — that is the one move it
exists to make visible.

The platform layer (`main.rs`, `render`, `dev`) is deliberately unfloored, and
headless Linux CI **skips every GPU test** (`Renderer::new_headless` returns
`None` with no adapter). A render change proven only on Linux CI is not proven;
macOS and Windows in `build-test-cross` are where it is really exercised.
