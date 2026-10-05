# Cloud brief: the standing rules

Every cloud coder the batch launches reads this file (#568). Its routine prompt
carries only step 0 (which runs before this file is read), a pointer here, and what
is specific to its issue. When a lesson changes how cloud coders must work, change
it **here**, once, not in the next prompt.

**Nobody can talk to you; the pull request is your only report.** Never send the operator a push notification: a hand-back or a decision goes in the PR description and an issue comment, where the orchestrator reads it and decides. Most "needs your call" questions are the orchestrator's to answer, and a phone alert at night is not how the batch works. Use the GitHub
MCP tools (`issue_read`, `create_pull_request`, `update_pull_request`,
`add_issue_comment`); load them with ToolSearch.

**A bug you find is yours to deal with.** Fix it in this branch when it's in
your way or small, or file an issue with the evidence (the `issue-write` skill)
and keep going. Either way it never goes unrecorded: the operator expects coders
to file issues mid-batch.

## Check the issue's blockers yourself

Before writing code, read the issue body for "Blocked by" and check each blocker's state on GitHub. The orchestrator checks GitHub's recorded relationships, and a blocker that exists only in prose slips past it. If a blocker is still open, stand down as the ~3-attempts rule says (comment on the issue, no branch) and say which issue should go first. (#399 on 2026-09-30.)

## Read first

`CLAUDE.md`, your issue(s), and `.claude/skills/ci-merge/SKILL.md`. The prompt may
name more.

## Your job ends at "ready"

A coder's session ends when its pull request is **ready**. Everything after that
(waiting on CI, rebasing, merging, reading a mutation report) belongs to the
orchestrator and the merge queue (#823). There is no check-in to schedule and
nothing to wait for.

- Run builds, tests and `make gates` in the **foreground**, with long timeouts,
  split across calls when needed. A session that starts a build in the background
  and ends its turn to wait is never woken.
- **Never schedule a `send_later` check-in.** Each one is a fresh cloud session
  reading state the orchestrator already has, and on 2026-10-04 check-ins were
  most of the late churn: green branches sat in draft waiting on them, and one
  pushed to a branch the queue already held.

## Ready means finished

- Mark the PR ready **only when the branch is truly finished**: the orchestrator
  merges a ready PR the moment its CI is green.
- For a proof run on a draft, dispatch the workflow on the branch instead of
  readying: `gh workflow run ci.yml --ref <branch>` (without `gh`, the GitHub MCP
  `actions_run_trigger` does the same). It runs the gates and nothing else.
  Never dispatch `mutants-sweep.yml`: that is the full sweep, five runners for
  most of a day the batch's gates need (#705).
- **A branch that adds mechanism asks for a scoped mutation run** (#750), and
  readies **in the same step**: dispatch `mutants-on-request.yml` on your branch
  with input `scope` set to `diff` (or path globs like `src/physics/**`) —
  `make mutants-remote SCOPE=diff` with `gh`, or the GitHub MCP
  `actions_run_trigger` without it — then mark the PR ready and end. Say in the
  PR that you dispatched it. The orchestrator reads the report when it finishes
  and launches a follow-up only for survivors in code this branch wrote.
- **Once ready, the branch is the merge queue's. Never push to it again**, not
  to rebase and not to fix your own red CI: a red or conflicting ready PR comes
  back to the orchestrator as a hand-back, and it briefs a fix. Anything you
  notice after readying goes in a comment on the issue. (#560 on 2026-09-30: a
  test-only push landed mid-queue and the queue handed the PR back.)

## The repo's rules that trip cloud coders

- `tests/` is **one integration binary** rooted at `tests/main.rs`; a new test file
  is a module under it. Test files are capped at **150 lines** (source at 300).
- A test that renders is named `gpu_*` (in-crate) or lives under `tests/gpu/`
  (integration). Install lavapipe to run them:
  `sudo apt-get install -y mesa-vulkan-drivers libvulkan1`.
- **An editor-visible PR attaches captures.** If the branch changes what the editor
  draws (a panel, an inspector card, the theme), run `make editor-capture` on `main`
  and on the branch (`ARGS="--select <entity>"` to open its inspector) and show both
  in the PR description, the way `docs/testing.md` § Editor captures says. Never
  leave a PNG on the branch that merges. (#731; #725 did it by hand.)
- `make gates` (foreground) before readying.
- If `cargo nextest` is missing in the container, install the prebuilt binary
  (`curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ~/.cargo/bin`)
  rather than falling back to `cargo test`: CI runs nextest, and its per-test
  process isolation and the `gpu` test group are what the gate measures.
- Rebase onto the latest `origin/main` before readying.
- Don't run `cargo mutants` locally; ask for a scoped run (above).
- **A refactor that claims no behaviour change proves it by comparison**:
  capture the output before (a scene dump, a bake, a capture, a test's printed
  values), and show it identical after, in the PR.
- **Bless by name.** Re-bless a snapshot or capture by its test name, never all
  at once, and look at every PNG you bless.

## What the description carries

Beyond what changed, why, the effect and the decisions:

- **A `Gates` line:** what ran green, on which head commit, and what didn't run
  and why (no real GPU, a human check). After a rebase it is re-run or marked
  stale.
- **A `Seam` section** when the PR creates something later branches build on:
  the types, functions, files and invariants a sibling should use. The
  orchestrator's next brief points at "PR #N, *Seam*" instead of re-describing
  merged work, so write it for a coder who has never seen your branch.

## Protocol

- Branch as the prompt names it, off the latest `origin/main`.
- Open a **draft** PR on the first commit. Title carries `(#N)`. Body: what
  changed / why / effect / decisions, `Closes #N` (one line per issue), and it ends
  with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- **A PR that leaves its issue open writes `Refs #N`, never `Closes #N`.** GitHub
  closes on the keyword whatever the prose around it says: on 2026-10-04 PR #777
  explained that #769 "stays open until one is chosen" and still closed it on
  merge. The match ignores formatting, so never write the keyword next to a number
  you don't mean to close, even quoted or in backticks: the retro PR that added this
  rule closed #769 a second time by quoting it.
- Commit messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Push often: a dead container takes its uncommitted work with it.
- Green gates and a rebased branch → mark the PR **ready for review**, and end.
- **Do NOT merge.**
- ≈3 attempts at the same failure, or a decision that is genuinely the user's:
  leave the PR as draft with the reason in its description, comment on the issue,
  and stop.
