---
name: issue-write
description: Write an issue for this repo — what it must contain, which labels it carries, and when Claude may file one unprompted. Use when filing an issue, splitting an idea into issues, or deciding whether something noticed mid-work deserves one.
---

# Writing an issue

The unit of work here is a well-specified issue. A future Claude reads it **cold**
and says *"I understand the assignment, I know how to proceed."* That is what lets
an issue run unattended, overnight, with nobody to ask.

## What it contains

- **What** — the change, concretely.
- **Why it belongs in rusty** — the project-level argument, the same value the
  label classifies. This is the half that survives; an issue whose reasoning is
  written down can be re-judged when circumstances change, and one without it can
  only be obeyed or ignored.
- **The roadmap** — the shape of the work, *not* the implementation intrinsics.
  Name the decisions the implementer must make and leave them theirs. Say what is
  explicitly **out of scope**; a boundary stated once saves an argument later.

**Evidence beats assertion.** An issue that quotes a measurement, a gate's own
output, a real file on disk or a line of the codebase is one nobody has to
re-derive. "`docs/api-faithfulness.md` lists `SetX` as ✅ and no system reads that
field" is worth more than "the API should be honest". So is a surviving mutant the
`mutants-pr` comment named, a module sitting on its floor in
`coverage-baseline.txt`, a line still in `tools/lint/parity_baseline.txt`.

**Cite what it relates to.** Sibling issues, the pull request that exposed it, the
rule in `CLAUDE.md` it turns on, the doc it makes false. A future reader arrives
with no memory of today.

**Unity is the yardstick, not the spec.** "Unity has it" is not a *why* — rusty is
a deliberate subset. The argument is what the eventual shooter needs and cannot get
today.

## The three gates

An idea becomes an issue only when all three hold. If any fails, **push back
instead of complying**:

1. **Understanding** — the intent is clear and can be restated. If unsure,
   restate it and confirm; do not guess.
2. **Value** — real value to the project. No busywork, no features for their own
   sake.
3. **Craft** — Rust/Lua good practice and the decided architecture: the five kinds,
   the one API surface, determinism in the sim, no event bus, no plugin trait, no
   new first-class component where a script component does. If the idea violates a
   settled decision, say so and propose the right shape.

## Filing what you notice

Claude may open an issue autonomously, and should, for anything that will recur
or that a tool would solve more than once. Only when the benefit outweighs the
cost of building it.

**A bug is always filable**, and that test does not apply to it — the question is
whether something is worth *building*, never whether a defect is worth *recording*.
If the bug questions a decision or exposes a foundational crack, tell the user;
that is a judgement call. Otherwise keep it brief and carry on.

The strongest issues come from doing the work: a mutation survivor that turned
out to be a real gap, a setter with no read-site, a doc claim the drift gate does
not cover and that quietly became false, a gate whose suggested remedy misled.
Those are findings, and findings are cheap to lose.

**File rather than fix** when the thing found is outside the branch in hand.
A branch that grows to cover everything it noticed is a branch nobody can review.

**Do not file and start in the same breath** — unless the work is a direct
consequence of an already-decided issue. An idea still being shaped has to settle
before anyone codes it. (This is rusty's rule and it is stricter than "absent a
stage label, it is startable"; both are in `CLAUDE.md` and this one wins.)

**Assign the user the moment work begins.** Unassigned means fair game; assigned
means in progress.

## Labels

**`plan` is the stage label, and its absence means ready.**

- `plan` — still being discussed with the user. **Never started**, by any means.
- `human` — needs a human in the loop end to end. It is a type label here, but
  **treat it as not-ready**: do not start it.
- *(neither)* — anyone can tell an agent "do issue N", subject to its blockers and
  to the file-and-start rule above.

**The judgement lives in the label**, so put it on honestly. Broad or vague is what
`plan` is for. A Claude-written issue **must** carry `plan` if it is a breaking
change, changes human-facing behaviour, needs a judgement call, or proposes a
structural change.

A `bug` usually should **not** carry one — it is specific, the deciding already
happened when the code broke, and nothing is gained by making it wait.

Type labels, combinable with `plan`:

`architecture` (the engine's own shape: a module, a convention, a guardrail baked
into the design — the determinism rule, the five kinds, the four-axis component
gate) · `infrastructure` (guardrails on the *development process*: CI, the lint
xtask, the headless harness) · `bug` · `documentation` · `foundation` (makes the
**engine** more complete) · `feature` (makes the eventual **game's** development
faster or easier — the game is built elsewhere, so engine-completeness outranks it)
· `human`.

The architecture/infrastructure line is the one that gets mislabelled: a guardrail
in the engine's design is `architecture`, a guardrail on how we build it is
`infrastructure`.

## Priority

**architecture → infrastructure → bug → foundation → feature.** `documentation`
never waits its turn.

That order is what to do **next** — and because merging is the serialized
bottleneck, it is felt hardest as merge order. Two branches already in flight do
not get re-ranked; the next one to start does.

## Relationships

Use GitHub's **Blocked by / Blocks**, and **sub-issues** when one is literal
groundwork for another. Link when one lays groundwork, makes the next
meaningfully easier, or would conflict too much if done concurrently.

**The dependency graph is the plan** — there are no rigid batches.

**Do not split for parallelism.** Sub-issues that all touch the same type are one
issue; see the `issue-batch` skill for why that costs more than it saves. A new
first-class component is the standard trap: its four axes look like four issues
and are one branch.

If a `plan` issue would affect how another is implemented or thought of, mark
that other one **blocked by** it.

## Closing

Reference the issue from the pull request that closes it — and **check the number**.
A typo'd `Closes #N` closes the wrong issue or none, silently, and nothing
verifies it.
