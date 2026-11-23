# The shepherd's steps

The `shepherd` agent (`.claude/agents/shepherd.md`) runs these, one turn of one pull
request's review loop per session. They are this skill's *Dispatch* and *Answering
the findings*, done by an Opus session in place of the orchestrator, so read
`SKILL.md` beside this file first: *When*, *Level*, *Steering, by nature* and
*Answering the findings* are the rules; this file is the order you apply them in.

**Your role is the reviewer's** (`SKILL.md`, *Why it exists*): was the job done, and
did the pull request make a new problem. You propose no improvement and look no
further. You never saw the brief or the coder's reasoning, which is the point: you
are the outsider.

## 1. Read the standing context

The root `CLAUDE.md` and every `CLAUDE.md` in a directory the diff touches; this
skill; `.claude/skills/independent-review/notes.md`, the batch's standing notes the
orchestrator keeps; the pull request's description, its issue and the issue's
comments (REST: `gh api repos/<owner>/<repo>/issues/<i>/comments`; `gh pr` and `gh
issue` answer 403 in a cloud session). Nothing else carries over between turns.

## 2. Find where the loop stands

`make shepherd PR=<n> ARGS=--stage` prints one line:

- **`stage: stop: <why>`**: post nothing, and end with that line. A draft, a
  `hold`, a verdict that still holds for the head, a record with no verdict line
  (someone decided by hand) and a closed pull request all stop here.
- **`stage: review`**: no record yet; go to step 3.
- **`stage: reread`**: a verdict was given on an older head and commits of its own
  followed (a round came back); go to step 5. `rounds sent: K of 2` says how many
  `round` verdicts it has had.

## 3. Review (the first pass)

1. **Review or not.** `SKILL.md`, *When*: a pull request under *No review* gets the
   record `No review: <why>` and, with no finding to hold it, `queue`; go to step 4.
   One whose diff changes only comments gets the check in
   `independent-review/comment-only.md` instead of a review.
2. **The level**: `make review-level PR=<n>`; raise it to `high` when the diff
   touches a stored format, determinism, the scripting sandbox or the merge path
   that its paths missed; never lower it.
3. **Review it yourself**: follow `SKILL.md`'s *Prompt template* as your own
   instruction, with the steering lists the diff hits, and run the `code-review`
   skill on the pull request number at that level, without `--comment` or
   `--fix`. Read the pull request detached (`git fetch origin pull/<n>/head && git
   checkout --detach FETCH_HEAD`); build nothing.
4. **Check each finding against the code**, then sort it (`SKILL.md`, *Answering
   the findings*, step 0): must-fix, dropped (in a word), or outside the branch.

## 4. Decide and post

**The verdict:**

- no must-fix finding: `queue`;
- must-fix findings and fewer than two rounds sent: `round`;
- must-fix findings after two rounds: `hold`; a third round is never sent;
- **`hold`, with each item on a `Sent up: <the item, word for word>` line**: a
  finding that questions a decision or a convention (root `CLAUDE.md`, *The
  approval test*), and anything you are unsure of. You decide nothing of these.

A defect outside the branch that someone can reach as a crash, data loss, a sandbox
escape or a determinism break is one comment, classed (`SKILL.md`, step 2), on the
open issue titled *Findings awaiting the operator* (find it by paging the open
issues over REST); name it in your return. Anything less is dropped. Never an issue.

**Post, in this order:**

1. Write the record's text to a scratch file: first line `Independent review` and
   the level and why (or `No review: <why>`), then one line per finding with its
   outcome; what only a human can do before the merge is a `Needs: <what>` line.
   Report, never instruct ("sent back to the coder", not "fix this").
2. **A `round` only:** post the `Brief` on the issue: first line `Brief`, then
   `Round: review`, the branch, the must-fix findings, each with its path,
   symbol, defect and failure scenario, and the round's standing line: **add
   commits on top, merge `origin/main` in only if the pull request conflicts,
   never rebase** (`issue-batch`, *The brief is a comment on the issue*).
   Nothing else: the coder reads the issue and the pull request itself.
3. `make shepherd PR=<n> ARGS='--record <file> --verdict <word>'`, with `--brief
   <the Brief's comment URL>` on a `round`. It reads the loop's stage again and
   refuses when it is now `stop` (another session posted first, or someone
   decided by hand): then post nothing and end. Otherwise it prints the record
   with its `Round brief:` and `Verdict:` lines on the head it read: post
   exactly that, as a comment on the pull request (the GitHub MCP
   `add_issue_comment`).
4. **A `round` only:** flip the pull request to draft (the GitHub MCP
   `update_pull_request`, `draft: true`). `make queue-quiet` relaunches the coder
   from the record's `Round brief:` line; you do not.

## 5. Reread (a round came back)

Ask only whether the new commits deliver what the last record sent back, and whether
they made a new problem. Read the record and `git log --no-merges <verdict
sha>..<head>`. **When the round changed code**, dispatch a fresh `reviewer`
(`SKILL.md`, *Dispatch*: the `Agent` tool, `subagent_type: "reviewer"`, no `model`,
the template's prompt) on the pull request, and check its findings as in step 3.4.
A round that changed only tests or wording you read yourself. Then step 4, with a
record headed `Independent review` again: each earlier finding fixed (its commit) or
not, each new one.

## Never

- Edit, push or merge; add the `queue` or `hold` label; send a push notification.
- Type a `Verdict:` line yourself, or post a record `--record` refused.
- Dispatch a `fork` or a `general-purpose` agent as the reviewer.
