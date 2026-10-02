---
name: ci-merge
description: Take a finished branch through to merged — the gates, CI, verifying a run happened on the head commit, rebasing, and triaging the mutation and coverage signals. Use when a branch is ready, when a pull request has gone red, when a mutation report needs triage, or when merging anything into `main`.
---

# Getting a branch merged

The steps, and the traps. `CLAUDE.md` carries why these exist; this is how.

**The gate list is `make gates`.** The `Makefile`'s `GATES` list mirrors
`.github/workflows/{ci,lint}.yml`; `make help` names each gate.

## Before marking a pull request ready

All of these must be green — one command, fastest-failing first:

    make gates

It refuses to start when cargo's target dir is outside this worktree, and checks
its own gate list is complete before running it.

**Both feature sets, always.** Half this crate is behind `dev` — the harness, the
session, the MCP bridge, `Debug.*` — and a default-features-only run compiles none
of it. The two clippy passes and the two test passes are not redundancy. The third
clippy pass (`--no-default-features`) is the editor-free player build (#431): the
only compile without egui.

**Some hard gates hide inside `cargo test`.** `tests/api_doc_drift.rs` and
`tests/callback_doc_drift.rs` fail the build when `docs/api/` (the API reference)
disagrees with the live Lua surface *in either direction* — an undocumented
binding and a documented-but-absent one both redden CI. They are dev-only, so
only the `--features dev` run sees them.

**The commit hook is not a substitute.** `.githooks/pre-commit` runs formatting
and the size gate only — no clippy, rustdoc, tests or dev feature set — and only
once `make setup` (or the cloud session-start hook) has pointed
`core.hooksPath` at it. Assume nothing heavy was caught for you.

The full list is deliberately **not** run before every push. Checkpoint commits
stay cheap.

**Failures land in `.lint/report.txt`** — read it rather than re-running the tool
to see what it said.

## The merge, one branch at a time

`make queue ARGS=--watch` (#664) is how a batch runs it: started once, it takes
every open pull request as it turns **ready**, highest priority first —
infrastructure → architecture → bug → foundation → feature, read from the pull
request's labels and the issues it closes, then unlabelled, oldest first,
Dependabot after all of them — and **exits** on the first hand-back, on the
machine failing (below), or when nothing is left (no open pull request, or only
drafts and handed-back heads, none moved for `--idle` minutes, default 90). A
handed-back head is remembered (under the checkout's git directory) and passed
over by later watches until it moves. It composes with every flag below; a
batch runs `ARGS="--watch --no-check --for 110"`. **`--for MINUTES`** (#697) is
the watch's own deadline: once it passes, the watch takes no new pull request,
finishes the one in hand (merged or handed back) and exits cleanly with a last
line `watch ended: deadline reached (…); N merged, nothing in hand.` That exit
is not a report — **relaunch the watch** with the same arguments. It exists
because a background command is killed at two hours wherever it is, a push or
a merge included; 110 keeps the deadline under that cap, so every exit lands
between pull requests. The deadline is checked only before a take, so a take
started at minute 109 still runs to its end — usually minutes, at worst its
own `--deadline` (45) of CI waiting. `make queue PRS="a b c"` takes named
pull requests instead, in the order given. Either way, the loop is one pull
request at a time — so no agent sits through a ten-minute run holding a
worktree open:

1. Rebase onto the latest `origin/main`, in a throwaway detached worktree it
   creates and removes (never one an agent is standing in).
2. Compile-check the rebased tree before pushing it (#571): `cargo check
   --locked --all-targets` with `dev` and with `--no-default-features`, then
   the size gate, building into the queue's own `target/merge-queue`. A clean
   textual rebase still breaks when a merge ahead changed a signature. Skipped
   for a Markdown-only branch, Dependabot, and `ARGS=--no-check`.
3. Push with `--force-with-lease` against the head it started from — both
   skipped when the rebase moved nothing, because the run on record is then a
   run on this exact commit.
4. Wait for the runs **on the rebased head** to exist and conclude.
5. Ask `make mergeable`'s judgement of that head — see the next section.
6. Squash-merge with the house-style title (`Title (#issue) (#pr)`) and
   `--match-head-commit`, so a push after the verdict makes GitHub refuse.

It **hands back** — names why; with named pull requests it skips that entry and
carries on, under `--watch` it exits — on a
conflict (naming the paths; it never resolves one), a rebased head that fails
the local check (never pushed), a red run, a run that never
appears, a head somebody else moved, a draft, or a merge GitHub refused. A 502 on
the merge call is followed by asking whether it merged. Exit status is non-zero
if anything was handed back.

It **stops the whole queue** — one message, the rest named *not taken* — when a
check fails for the machine's reasons: out of disk or memory, or rustc or the
linker killed (#580). Nothing is known to be wrong with that branch, and every
later one would fail the same way. **Run the queue from a worktree under
`.claude/worktrees/`, never the scratchpad**: the scratchpad is a tmpfs, a cold
`target/merge-queue` build filled it and three healthy branches came back as
broken. The queue refuses to start when its target directory is on a tmpfs.

What it leaves to you: its local check only ever *refuses* a push, never grants
a merge (`make gates` before readying is the author's job), it never reads the mutation or coverage signal, and **removes no
worktree and deletes no branch** — the summary lists the merged ones. Remove each
worktree (~2.5–4.5 GB of `target/` each) and its branch once nobody is standing in it.

- `ARGS=--dry-run` reads everything and computes the rebase locally, but pushes,
  comments and merges nothing — use it to see what a queue *would* do.
  `ARGS=--no-merge` does the real rebase and wait, and stops at green.
- **Dependabot branches are never force-pushed** — Dependabot stops maintaining a
  branch someone else pushed to, and it rebases (and cancels its own runs) by
  itself. The queue comments `@dependabot rebase` when one is behind, waits for a
  head on `main`'s tip, and judges that.
- By hand, the same loop is: rebase in the branch's worktree, `cargo check`
  both feature sets and `make size`, push with `--force-with-lease`, wait,
  `make mergeable PR=N`, squash-merge. Prefer the queue.

Merging is serialized because rusty is **one compiled crate**: two branches can
each be green alone and break `main` together. A rename, a changed signature, a
moved module — no textual conflict catches any of them, and being a single crate
makes it *more* likely, not less, because everything is in scope of everything.
The queue does not change that; it only changes who waits.

### The Markdown exception, and the file it does not cover

CI's `changes` filter is `code: - '!**/*.md'`. A pull request touching only
Markdown skips the heavy *steps*, but the gating jobs still **run** and report
`success` — a job whose steps all skip still succeeds. That is deliberate, and the
workflow comments say why: a ruleset reads a *skipped* required check as
unsatisfied, not as a pass, so skipping the job outright would wedge a docs-only
pull request. So a docs-only pull request needs no serialization and merges
freely.

**`docs/api/` is not one of those.** Two hard-gate tests parse it, so
a Markdown-only edit to it can break `main` on its own. CI's `code` filter lists it
explicitly (#525, #569), so a pull request touching it runs the drift tests like any code
change, and it is serialized like code. Same for `docs/api-faithfulness.md`'s
catalog if a branch is relying on it: treat it as code.

## Verify the run happened on the head commit

    make mergeable PR=N

Exit 0 means CI genuinely ran on the head commit and passed; anything else prints
why. It is the last thing before any merge, the queue asks it too, and **on rusty
it is the only check there is**: `main`'s one ruleset rule is *deletion* — no
required status checks (#491, a human task), no merge queue — so GitHub will
happily merge a red or entirely unbuilt pull request. Confirm before trusting
otherwise:

    gh api repos/:owner/:repo/rulesets --jq '.[].id' \
      | xargs -I{} gh api repos/:owner/:repo/rulesets/{} --jq '.name, [.rules[].type]'

What it asks, so its answers read plainly (`.github/scripts/mergeable.py` has the
incidents behind each):

- **Both workflows, on the head SHA.** `ci-gate` (from `ci.yml`) and `lint-gate`
  (from `lint.yml`) each collapse their workflow's gating jobs; a green `ci` beside
  a missing `lint` is half a check.
- **A gate that passed is not evidence anything ran.** Both gates run
  whenever the workflow was not cancelled (`!cancelled()`, #515) and pass over
  *skipped* jobs, so a draft's run is gate-green with
  nothing compiled. A run counts only when a gated job itself succeeded.
  `changes` succeeding proves nothing — it runs on drafts too.
- **Every run on the commit, not the first listed.** A red run beside a green one
  refuses. A **cancelled** run is ignored only when a run of the same workflow
  on the same commit that was *not* cancelled is still going (then the answer is
  "wait") or was created no earlier — same second included, since neither
  timestamp nor run id orders two same-second runs (#562; #482 cancels
  superseded runs). A cancelled run nothing replaced refuses, because nothing
  has answered.
- **No run at all** is told apart: a branch that conflicts with `main` gets no run
  (GitHub cannot build a merge ref, so it creates no run, check or error — do not
  edit the workflow), versus a pull request readied moments after a push that lost
  its run (force one with an empty commit). An invalid workflow *does* produce a
  run, a `startup_failure`.
- **A Markdown-only change to `docs/api/` is refused** unless
  `build-test`'s dev-feature test step really ran. CI's `code` filter includes that
  directory since #525, so on a fresh run it does; the check stays as the guard.

It ignores what is not a gate: `main-health` (a `workflow_run` on `main`), `docs`,
and the coverage and mutation jobs. `mutants-pr` and `coverage-pr` run *inside*
the `ci` run but outside `ci-gate`'s `needs:`, so a `ci` run still in progress
counts as settled once `ci-gate` has concluded — a signal still running or red
never holds `make mergeable` or `make queue` (#555). Its own tests run as `make scripts`, a gate, and in `lint.yml`.

`gh pr checks N` is **not** a substitute. It has no commit column, so it cannot
answer "did this run build the head I am about to merge?", and it blends runs — a
skipped job sits in the same list as a real one, and `changes` appears **twice**,
once per workflow. To look by hand, ask about the commit:

    SHA=$(gh pr view N --json headRefOid -q .headRefOid)
    gh api "repos/:owner/:repo/commits/$SHA/check-runs" \
      --jq '.check_runs[] | "\(.name)\t\(.status)\t\(.conclusion)"' | sort

**Never hand-roll a "wait for CI" loop that treats zero checks as success.**
Absent and passing are different states; a loop counting non-completed checks
finds zero of each. `make queue` is that loop, written so it cannot.

*(GitHub's **native** merge queue would replace `make queue` — both workflows
already trigger on `merge_group` with collapsed gates — but it needs an
organization-owned repository, and moving the repo is the user's call, #486.
`make mergeable` stays useful either way.)*

## A red ready pull request stays ready

Fixed in the next commit; it does not go back to draft. Draft is for work that is
genuinely unfinished, blocked, or handed over — including the hand-back after ≈3
attempts at the same failure, which does go to draft, with the reason in the
description.

## Rebasing

Expect conflicts wherever every feature appends: `Entity`'s `Option<…Component>`
fields, `ComponentKind`'s variant list, `api/mod.rs`'s module
list, `app/registry.rs`'s ordered `register` calls, the Add Component menu,
`docs/api/index.md`'s namespace list, `scripting/callbacks.rs`, and the burn-down
baselines where both sides *removed* lines.

`make queue` hands every one of these back rather than resolving it.

**Two authors both being right is the common case**, and the resolution is usually
to keep both sides, ordered deliberately rather than by merge accident. One of
those needs more than that: `app/registry.rs`'s `build()` is the **only** place
per-frame system order is defined. Merging it by textual proximity silently
reorders the schedule.

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

- **A binding change updates its namespace's page in `docs/api/`.** Existence parity, both
  directions, dev feature set. Signatures are out of scope.
- **A new lifecycle callback updates the callback section of `docs/api/index.md`**, against
  `src/scripting/callbacks.rs`.
- **A new first-class component satisfies all four axes** — the `Entity` field,
  the Add Component entry, an inspector card, an `src/api/<x>.rs` namespace
  registered and documented — or gets a `WAIVERS` row in
  `tools/lint/src/components/waivers.rs` with a written rationale. `--components`
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
| mutation | `mutants-pr`, `--in-diff`, sticky comment | `mutants`, nightly, full sim in 10 shards, `mutants-report` summary + artifact; skipped on a night `main` has not moved (the last finished report still stands) |
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

The platform layer (`shell`, `render`, `dev`) is deliberately unfloored. GPU
tests run on all three CI OSes — Linux through Mesa's **lavapipe** software
Vulkan driver (#489) — and `RUSTY_REQUIRE_GPU=1` fails the run if an adapter
ever goes missing, so a green run means they rendered. Two of the three
adapters are software (lavapipe, WARP): a change that depends on real-GPU
behaviour is proven on macOS's Metal, not by a green Linux job alone.
