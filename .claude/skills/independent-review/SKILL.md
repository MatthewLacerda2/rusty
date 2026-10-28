---
name: independent-review
description: Use before a rusty PR gets the `queue` label when it implements engine mechanism (renderer, shaders, physics, navigation, animation, the sim, scripting, the API surface), touches a stored format or determinism, changes CI or the merge path, or is a refactor — and whenever the user asks for an open or merged PR to be reviewed. Dispatches a fresh, read-only reviewer that never saw the coder's reasoning, then has the coder answer its findings.
---

# Independent Review

## Why it exists

The operator does not read the code (root `CLAUDE.md`, *Unity is the
reference*), so besides the gates a PR gets one outside read: this one. A coder
reviewing its own diff reviews what it meant to write, not what it wrote, and
the orchestrator who briefed it shares the brief's blind spots. The reviewer is
a separate agent that reads the PR the way an outsider would, and it is never
told what to think of it. **Everything below exists to keep the coder's bias out
of the reviewer.**

The review itself is Claude Code's `code-review` skill. It ships with Claude
Code and moves with every new model, so this skill only says when to run it,
how to dispatch it without bias, and what to do with what it finds; it never
restates how to review. (Adopted from the operator's `prototipo-atem` repo,
2026-10-19.)

## When

Before the `queue` label (`issue-batch`, the orchestrator's read), when the PR
does **work the engine does itself**:

- **engine mechanism**: the renderer and `engine/shaders/`, physics,
  navigation, animation, the sim's systems and resources, the scripting VM and
  its sandbox, procgen and shadergen;
- **the API surface**: a Lua namespace added, or a function's behaviour changed;
- **a stored format or determinism**: the scene, prefab or save format, an asset
  cache or sidecar, anything a sim module reads that could carry a clock, an
  unseeded RNG or iteration order;
- **the development machinery**: `.github/workflows/`, the `Makefile` gate
  verbs, the hooks, the merge path (`.github/scripts/merge-queue.py`,
  `merging/`, `mergeable.py`), or how a `tools/lint` gate decides;
- **a refactor**: code moved or reshaped across modules, whatever its size.

**No review** for what is standard, mechanical, or proved by a gate alone:
documentation; the editor's UI and egui (the work `issue-batch` gives Sonnet,
*Which model*); a mechanical rebase; a size-cap split; a new lint rule whose
issue already names its flag and pass cases; adopting a cargo crate the way its
docs say, without reinventing anything; Dependabot bumps. When a PR is both, the
trigger wins.

And on demand: when the user asks for a PR to be reviewed, open or merged
(*Catching up on a merged PR*).

## Dispatch

- **A fresh local agent**: the `Agent` tool with `subagent_type:
  "general-purpose"`. **Never `"fork"`**: a fork inherits the orchestrator's
  context, and with it the bias this skill exists to avoid. **`isolation:
  "worktree"`**: `code-review` checks the PR out to read it, and in the shared
  main checkout that moved other agents' branch and left `prNNNN` branches
  behind (three times in the 2026-10-20 batch). Remove a reviewer's worktree
  only after its report is in, by its own path, never by a pattern: a blanket
  `agent-*` cleanup removed a running reviewer's checkout and lost its report
  (2026-10-23). No model override: it runs on
  the current model, Opus (read-only agents stay on Opus, `issue-batch`,
  *Which model*).
- **Read-only, and no build.** It reads the PR, the repository and the issue,
  and runs read-only commands (`gh pr view`, `gh pr diff`, `gh issue view`,
  `git show`). It makes no edits, commits, pushes or comments, and it does not
  compile: the gates and CI already prove the build, and a review costs this
  machine nothing.
- **The prompt carries only what an outsider would get:** the PR number, the
  issue it closes, the steering for the PR's nature, and the template below.
  **Not** the brief, a summary of the change, the coder's rationale, what was
  already checked, which differences are "known and fine", or what you expect
  it to find. The PR description is fair game (it is what the author tells
  every reader), and the reviewer tests its claims rather than trusting them.
- **One reviewer per PR**, all dispatched in the same message when there are
  several.

### Level

`code-review` takes an effort level; rusty uses only **`medium`** and **`high`**
(operator, 2026-10-19). **The reviewer picks it, not the dispatcher:** it reads
`gh pr diff` first and sees what the PR really touches, where the dispatcher
only knows what the brief meant it to touch. The rule it applies follows what a
miss would cost, not the size of the diff:

- **`high`** when the diff touches a stored format (scene, prefab, save, asset
  cache), determinism, the scripting sandbox (what a script may reach), or the
  merge path. A miss there is a broken save, a desynced harness, an escape from
  the sandbox, or a queue that merges the wrong thing for every PR behind it.
- **`medium`** for everything else.

The rule goes into the prompt whole, and the reviewer names the level it chose
and why at the top of its answer. Called without one, `code-review` reuses
whatever level was typed last in the session, so a level is always passed.

### Prompt template

```
Review PR #<n> of MatthewLacerda2/rusty. It closes issue #<i>: read it with
`gh issue view <i> --comments`; it is what the PR is meant to deliver.
Read the root CLAUDE.md and any CLAUDE.md in a directory the PR touches first:
they hold the conventions the code must follow.

First read `gh pr diff <n>` and pick the effort level: `high` if the diff
touches a stored format (scene, prefab, save, asset cache), determinism, the
scripting sandbox (what a script may reach) or the merge path; `medium`
otherwise. Then run the `code-review` skill on PR #<n> (pass the PR number in
its arguments) at that level, without `--comment` and without `--fix`. Called without it, the
skill reviews the checkout's own local diff, which is not this PR: if what it
reviewed is not `gh pr diff <n>`, discard its findings and review that diff
yourself. You run in your own worktree: check out or fetch what you need
there, never in the main checkout (the repository root), which
other agents share. Read the PR **detached**, with no local branch name:
`git fetch origin pull/<n>/head && git checkout --detach FETCH_HEAD`. A named
branch (`pr<n>`, `pr<n>-review`) outlives your worktree in the shared
repository. You are read-only: no edits, commits, pushes or comments,
and no builds.

Look especially for: <the steering lists for the PR's nature, pasted whole>.

Start your answer with the level you chose and why, in one line. Then answer
with one entry per finding: path and symbol, the defect in one sentence,
a concrete failure scenario (inputs or state → wrong result or crash), and
whether it must be fixed before merge. Verify each one against the code before
reporting it. An empty list is a valid answer.
```

### Steering, by nature

Paste every list whose trigger the PR hits.

- **Engine mechanism**: edge cases at zero, empty and huge (no entities, a
  degenerate mesh, a Dust2-sized level); every frame stage it runs in and the
  order modules `register`; what a hot-reload, Play/Stop snapshot or scene load
  does to its state; GPU resources freed and rebuilt on resize; work per frame
  that should be per change; Unity's behaviour where the issue was silent.
- **What is drawn** (the renderer, `engine/shaders/`, materials, post effects,
  lighting, UI, editor panels): the PR description carries a `Looked at` line
  (`issue-batch/cloud-brief.md`, #1483) naming the scene, what was rendered
  before and after and what was seen. You do not build, so you do not
  re-render: check only that the line is there and covers what the diff
  changes. A missing line, or one that skips a change the diff makes visible,
  is a finding.
- **API surface**: `docs/api/` matches what the code does, not only what exists;
  errors a script can trigger reach the console instead of a panic; argument
  shapes match the namespace's siblings; the editor can do the same thing.
- **Stored format / determinism**: no migration, legacy alias or compatibility
  shim is added (CLAUDE.md, *No backward compatibility*); what is saved is
  references and values, never GPU or derived state; iteration order, clocks
  and RNG seeds in anything the sim reads; the same seed and inputs give the
  same frames.
- **Development machinery**: a red gate still fails the job; triggers still
  fire (drafts, paths, Markdown-only PRs); required check names unchanged;
  `make gates` and CI still run the same command; the queue cannot merge an
  unchecked head, skip a PR silently, or loop; a script still runs from a cloud
  session.
- **Refactor**: same output for the same input; callers left on the old path;
  the public API unchanged; moved code still tested; dead code left behind;
  scope beyond the refactor; the layer table still true.

## Answering the findings

The reviewer can be wrong, so the orchestrator checks every finding against the
code first, then:

0. **Sort them first** (CLAUDE.md, *Issues are for what matters*). A finding is
   **must-fix** when it is a failure someone will hit on a real path: a wrong
   result, data loss, a crash, a regression, a doc that now says the wrong thing,
   a structural or tooling crack. A finding nobody would hit, a hypothetical edge,
   a test that could be stronger, a style nit: fold it in only when it costs a line,
   otherwise drop it, and don't file it. A reviewer told to look hard always finds
   more; most of it is noise for this project.
1. **Must-fix** findings go back to the coder: a local coder is resumed with
   `SendMessage`; a cloud coder gets a new `Brief` comment listing them and a
   fresh routine (`issue-batch`, *The brief is a comment on the issue*), on the
   model its task calls for. Anything declined gets its reason. A finding that
   needs a decision no rule settles goes to the user, and the PR goes to draft
   naming it.
2. **One PR comment, headed `Independent review`**, lists every finding and what
   became of it: fixed (with the commit), dropped (in a word), sent to the user,
   or filed as an issue when it lies outside the branch and passes *Issues are
   for what matters*. No findings is said too. It is the record that the review
   happened. It reports, it never instructs: "sent back to the coder", never "fix
   in this branch" or where the new `Brief` is. A readied session still
   subscribed to the PR reads an instruction there as work, and races the fresh
   session (2026-10-26, #1447).
   **Before filing a pre-existing finding, class it** (`issue-batch`, *Filing a
   bug*): look for earlier cases of its kind; a case is filed with `Kind: case —
   searched <terms>`, a symptom gets the fix that scales as a root issue, with
   the finding as its sub-issue. A finding of a kind that already has an open
   root issue goes **on that issue**, as a comment or a sub-issue, never as an
   issue beside it.
3. When the fixes rework the logic substantially, one more reviewer reads the
   new diff. Otherwise one round is the review.
4. Then the `queue` label. The review does not replace the gates or CI; it runs
   before the queue, never inside it.

## Catching up on a merged PR

Same dispatch, on the merged PR's number. Its fixes go in a **new PR** off
`main` (or an issue, when they are larger than a fix), whose description names
the reviewed PR; the `Independent review` comment goes on the reviewed PR and
links the fix.

## Common mistakes

- Dispatching a `fork`: it is the orchestrator again, with its blind spots.
- Pasting the `Brief` or a summary "to save it time"; it then reviews the
  summary, not the code.
- Letting the reviewer push its own fixes. It reports; the coder fixes.
- Picking the level for the reviewer, or leaving the rule out of the prompt.
- Letting `code-review` run without the PR number: it then reviews the
  checkout's local diff, some other change entirely.
- Labelling `queue` before the `Independent review` comment exists.
- A reviewer that checks the PR out into a named branch (`pr1316`,
  `pr1403rev`, …): the worktree goes, the branch stays in the shared
  repository. The Post Processing batch (#1446, 2026-10-26) ended with 39 local
  branches, most of them reviewers', and sweeping them by hand was refused as
  irreversible (#1482). The prompt says to read it detached; `make reclaim`
  deletes the ones that slip through.
