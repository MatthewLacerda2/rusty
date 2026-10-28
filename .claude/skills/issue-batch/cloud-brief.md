# Cloud brief: the standing rules

Every cloud coder the batch launches reads this file (#568). Its routine prompt
carries only a pointer here and the issue number; what is specific to the issue
is a `Brief` comment on it (#910). When a lesson changes how cloud coders must
work, change it **here**, once, not in the next prompt.

**Nobody can talk to you; the pull request is your only report.** Never send the operator a push notification: a hand-back or a decision goes in the PR description and an issue comment, where the orchestrator reads it and decides. Use the GitHub
MCP tools (`issue_read`, `create_pull_request`, `update_pull_request`,
`add_issue_comment`); load them with ToolSearch.

**A finding is yours to deal with**: fold it in when it's in your way or costs a
line; file it only when it passes CLAUDE.md's *Issues are for what matters* (it
already caused a problem, costs more to leave than to fix, or cracks the structure
or tooling), with the `agent` label (`planning` when it fails the approval test);
otherwise drop it.

**A bug you file is classed first** (`issue-batch`, *Filing a bug*, #1173): look
for earlier cases of its kind, open and closed (search answers 403 here, so page
the REST issue list and grep it); class it a case or a symptom; for a symptom,
file the fix that scales and make your finding its sub-issue; fold an instance of
an open root issue into that issue as a comment. The issue carries one `Kind:`
line: `Kind: #N`, `Kind: case — searched <terms>`, or `Kind: root`.

**Need a sibling's tool for your own proof? Patch it locally, don't commit it.**
If your PR's captures or measurements need something another in-flight issue is
building (a CLI flag, a capture option), apply the smallest uncommitted patch to
take them, and say so in the PR. A committed copy conflicts with the original.

## Check the issue's blockers yourself

Before writing code, read the issue body for "Blocked by" and check each blocker's state on GitHub; a blocker written only in prose slips past the orchestrator. If one is still open, stand down (comment on the issue, no branch) and say which issue should go first.

## Read first

`CLAUDE.md`, your issue(s), your brief, and `.claude/skills/ci-merge/SKILL.md`.
**Your brief is the newest comment on your issue whose first line is `Brief`**
(#910): your branch name, what to build on, the siblings whose files to stay
out of, and decisions already made. A later `Brief` supersedes an earlier one.
It may name more to read.

## Your job ends at "ready"

A coder's session ends when its pull request is **ready**. Everything after that
(waiting on CI, rebasing, merging, reading a mutation report) belongs to the
orchestrator and the merge queue (#823). There is no check-in to schedule and
nothing to wait for.

- Run builds, tests and `make gates` in the **foreground**, with long timeouts,
  split across calls as *The container* says.
- **Never schedule a `send_later` check-in**: it is a fresh session reading state
  the orchestrator already has, and it pushes to branches the queue holds.

## Ready means finished

- Mark the PR ready **only when the branch is truly finished**: the orchestrator
  reads it, labels it `queue`, and the watch merges it the moment its CI is
  green. **Never add the `queue` label yourself**: it is the orchestrator's
  go-ahead after reading your diff (#925).
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
  PR that you dispatched it. The report lands as a comment on your PR when it
  finishes (#912); the orchestrator reads it there and launches a follow-up
  only for survivors in code this branch wrote. One run per branch: a later
  dispatch on the same branch cancels the earlier run, and an unmerged close
  of the PR cancels what is left (a merged PR's run finishes and reports,
  #1530), so dispatch once, when you ready.
- **Once ready, the branch is the merge queue's. Never push to it again**, not
  to rebase and not to fix your own red CI: a red or conflicting ready PR comes
  back to the orchestrator as a hand-back, and it briefs a fix. Anything you
  notice after readying goes in a comment on the issue.
- **Readying ends you for good.** A later draft flip, a review comment or a PR
  comment pointing at a new `Brief` is not yours to act on: the orchestrator
  launches a fresh session for every re-brief. On 2026-10-26 a readied session
  followed a review comment to the new `Brief` while the fresh one did the same,
  and one force-push overwrote the other.

## The container

Lessons from cloud coders' run logs (rusty's and scorsese's, #924). Each one cost
a coder real time.

- **A tool call is moved to the background at ten minutes**, and you are woken
  when it finishes (MatthewLacerda2/scorsese#1500). A cold `make gates` in the
  container takes about 16, so split it anyway, one call each, so every result
  comes back in the call that ran it:
  1. `make quick-gates` (seconds): the self-checks and every gate in the Makefile's
     `QUICK_GATES`. It is a Makefile fact, not a list copied here, so a gate added
     to the Makefile is run; a hand-written list left `docrefs` out for PR #1177;
  2. `make clippy`;
  3. `make doc deny`;
  4. `cargo nextest run --locked --no-run` (warms the default test build);
  5. `cargo nextest run --locked --features dev --no-run` (warms the `dev` one);
  6. `make test` (now only runs: about 3 minutes).
  On a warm branch, after an edit, one call per gate is enough.
- **Run each heavy gate once, into a log, then grep it**:
  `make test > target/test.log 2>&1; echo exit=$?`, then `grep` the log for what
  failed. Re-running a gate to see a different slice of its output pays the
  whole run again.
- **Exit 137 means out of memory**, not a failed test: rerun with
  `CARGO_BUILD_JOBS=2`.
- **GitHub GraphQL and search answer 403** in the container. `gh` works over REST
  (`gh api repos/...`, `gh workflow run`, `gh label create`), and so do the GitHub
  MCP tools; `gh pr` and `gh issue` subcommands that use GraphQL do not. The
  repo's own scripts are REST-only (#970, `.github/scripts/ghrest.py`), so
  `make blockers`, `make mergeable` and `make queue` run here too.
- **The container cannot delete a branch** (neither `git push --delete` nor the
  REST refs API is allowed). A throwaway branch you pushed goes under the PR's
  *For a human* heading for the operator to delete.
- **`git status --short --ignored` lists nothing you wrote** before readying: a
  `.gitignore` pattern can silently keep a source file out of a commit.
- **A refused force-push has one fallback.** The container's permission check
  sometimes refuses `git push --force-with-lease` even when the brief authorises it
  (four times on 2026-10-26). Don't work around it: push the same commits to
  `<branch>-v2` (no force), open a ready PR with the old one's description headed
  "Supersedes #N (<why>; a force-push was refused)", and comment on #N. The
  orchestrator closes #N.
- **Rebase onto `main`, never merge `main` into your branch.** The merge queue
  rebases every branch before it merges it, and a rebase replays the commits from
  before a merge commit, so conflicts the merge resolved come back and the PR is
  handed back (2026-10-26, PR #1458).
- **Rebase once, last.** After a rebase over unrelated merges, `cargo check`, the
  size gate and your own tests are enough; the whole gate list again is not.
- **Blessings come from the container's Mesa, not CI's.** A capture or snapshot
  blessed here may differ from what CI renders: bless nothing from a container
  without saying so in the PR.

## The repo's rules that trip cloud coders

- `tests/` is **one integration binary** rooted at `tests/main.rs`; a new test file
  is a module under it. Test files are capped at **150 lines** (source at 300).
- A test that renders is named `gpu_*` (in-crate) or lives under `tests/gpu/`
  (integration). They run on lavapipe, which the SessionStart hook installs
  along with `cargo-nextest` and `cargo-deny` (#826). If its log
  (`session-start: <step> FAIL`) shows a step failed, do what that line says
  before trusting a green run: without the driver, GPU tests skip silently.
- **A branch that changes what is drawn renders it and looks** (#1483): the
  renderer, `engine/shaders/`, materials, post effects, lighting, UI, editor
  panels. Render a primitive scene that shows the change before (on `main`) and
  after (on the branch): the feature off and on, and its knobs at the extremes
  where those are part of the issue. Open every image with `Read`, and fix what
  is plainly wrong (a black frame, a mirrored flare, a square bokeh) before
  readying. The PR carries your word, not the pictures: a `Looked at` line
  (below), and **no PNG on GitHub**, not committed, attached or hosted. Images
  stay in your scratchpad, named with the issue number, and die with the
  session. How to render a pair in one command, and the trap that renders an
  effect as a no-op: `docs/testing.md`, *Looking at a change*. The editor is
  rendered with `make editor-capture` (`ARGS="--select <entity>"` opens an
  inspector), the same way.
- `make gates` (foreground, split as *The container* says) before readying.
- **Windows runs only when asked** (#1395): a branch that touches paths, separators,
  process spawning or other platform code dispatches `gh workflow run windows.yml --ref
  <branch>` and quotes the run in its PR. Nothing else runs Windows on a PR.
- Never fall back to `cargo test`: CI runs nextest, and its per-test process
  isolation and the `gpu` test group are what the gate measures.
- Don't run `cargo mutants` locally; ask for a scoped run (above).
- **A refactor that claims no behaviour change proves it by comparison**:
  capture the output before (a scene dump, a bake, a capture, a test's printed
  values), and show it identical after, in the PR.
- **Bless by name.** Re-bless a snapshot or capture by its test name, never all
  at once, and look at every PNG you bless.

## What the description carries

Beyond what changed, why, the effect and the decisions:

- **A `Gates` line:** what ran green, on which head commit, and what didn't run
  and why (no real GPU, a human check). After your own rebase it is re-run or
  marked stale; a rebase the merge queue pushes later dates it for you (#968).
- **A `Looked at` line** on a branch that changes what is drawn (above): for
  each before/after pair, the scene, what was rendered and what you saw, in one
  or two sentences. Say what you did *not* check (motion, Metal, cost, taste):
  those stay under *For a human*. Your look catches broken; the operator judges
  good. The independent review treats a missing line as a finding.
- **A `For a human` heading** for what only the operator can check (a real
  window, speakers, a repository setting, a branch to delete), each one a
  `- [ ]` item. The heading must *start* with "For a human": `make checks`
  gathers every unticked item under such headings from a batch's merged PRs for
  the batch report (#929), and finds nothing under any other name. When nothing
  needs a human, leave the heading out or write a sentence under it, never
  `- [ ] Nothing needs a human`, which would be listed as a check.
- **A `Seam` section** when the PR creates something later branches build on:
  the types, functions, files and invariants a sibling should use. The
  orchestrator's next brief points at "PR #N, *Seam*" instead of re-describing
  merged work, so write it for a coder who has never seen your branch.

## Protocol

- Branch as the prompt names it, off the latest `origin/main`. The container's clone
  can be a stale snapshot; the session-start hook fetches GitHub's `main` (its
  `session-start: main` line). If that line says FAIL, `git fetch origin main` before
  you branch.
- Open a **draft** PR on the first commit. Title carries `(#N)`. Body: what
  changed / why / effect / decisions, `Closes #N` (one line per issue), and it ends
  with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- **A PR that leaves its issue open writes `Refs #N`**, and never puts a closing
  keyword next to a number it doesn't mean to close, even quoted or in backticks
  (CLAUDE.md, *The closing-keyword trap*). The merge path refuses a keyword not
  alone on its line (#966), so write `Closes #N` on a line of its own.
- Commit messages end with a `Co-Authored-By:` line naming the model you run
  as, e.g. `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` (a Sonnet
  coder names Sonnet; the brief's `Model:` line says which, *Which model*).
- Push often: a dead container takes its uncommitted work with it.
- Green gates and a rebased branch → mark the PR **ready for review**, and end.
- **Do NOT merge, and do not review your own PR.** The independent review is the
  orchestrator's (`independent-review`): it dispatches a reviewer that never saw
  your reasoning, which one you spawn cannot be. A brief that says the PR "gets an
  independent review" means the orchestrator will run one. PR #1184's coder ran its
  own, and it had to be done again.
- ≈3 attempts at the same failure, or a decision that is genuinely the user's:
  leave the PR as draft with the reason in its description, comment on the issue,
  and stop.
