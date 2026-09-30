# Cloud brief: the standing rules

Every cloud coder the batch launches reads this file (#568). Its routine prompt
carries only step 0 (which runs before this file is read), a pointer here, and what
is specific to its issue. When a lesson changes how cloud coders must work, change
it **here**, once, not in the next prompt.

**Nobody can talk to you; the pull request is your only report.** Use the GitHub
MCP tools (`issue_read`, `create_pull_request`, `update_pull_request`,
`add_issue_comment`); load them with ToolSearch.

## Read first

`CLAUDE.md`, your issue(s), and `.claude/skills/ci-merge/SKILL.md`. The prompt may
name more.

## Never wait on background work without a wake-up

- Run builds, tests and `make gates` in the **foreground**, with long timeouts,
  split across calls when needed. A session that starts a build in the background
  and ends its turn to wait is never woken.
- If you must wait on CI, schedule your own check-in with `send_later` (the
  `Claude_Code_Remote` MCP tool) **before** ending the turn.

## Ready means finished

- Mark the PR ready **only when the branch is truly finished**: the orchestrator
  merges a ready PR the moment its CI is green.
- For a proof run on a draft, dispatch the workflow on the branch instead of
  readying: `gh workflow run ci.yml --ref <branch>` (without `gh`, the GitHub MCP
  `actions_run_trigger` does the same).

## The repo's rules that trip cloud coders

- `tests/` is **one integration binary** rooted at `tests/main.rs`; a new test file
  is a module under it. Test files are capped at **150 lines** (source at 300).
- A test that renders is named `gpu_*` (in-crate) or lives under `tests/gpu/`
  (integration). Install lavapipe to run them:
  `sudo apt-get install -y mesa-vulkan-drivers libvulkan1`.
- `make gates` (foreground) before readying.
- Rebase onto the latest `origin/main` before readying.
- Don't run `cargo mutants`.

## Protocol

- Branch as the prompt names it, off the latest `origin/main`.
- Open a **draft** PR on the first commit. Title carries `(#N)`. Body: what
  changed / why / effect / decisions, `Closes #N` (one line per issue), and it ends
  with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Commit messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Push often: a dead container takes its uncommitted work with it.
- Green gates and a rebased branch → mark the PR **ready for review**.
- **Do NOT merge.**
- ≈3 attempts at the same failure, or a decision that is genuinely the user's:
  leave the PR as draft with the reason in its description, comment on the issue,
  and stop.
