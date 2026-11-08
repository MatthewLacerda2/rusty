"""After a merge: the scoped mutation run its coder asked for (#1934).

Mutation is a signal, never a gate (CLAUDE.md, *Gates vs. signals*), but in a
batch it was costing the gates. Every cloud coder dispatched
`mutants-on-request.yml` on its branch in the same step as readying, so on
2026-11-08 six scoped runs held runners beside the weekly sweep's seven while
nine `ci` and six `lint` runs queued behind them, each gating run waiting
20-40 minutes to start with eight pull requests in the merge queue. GitHub
cannot rank one workflow over another, and the workflow's one-run-per-branch
group could only cap a branch, never the account.

So the coder **asks** and the queue **dispatches, after the merge**:

- **The ask is a line in the description**, `Mutation: diff` or
  `Mutation: <globs>` ([`ask.requested`]). A line rather than a label, so a
  coder with no label tools can still ask; an absent or unreadable one asks
  for nothing.
- **The run is on `main`, at the squash commit** ([`ask.inputs`]): `diff`
  becomes the commit's own Rust change against its parent (the
  `mutants-scope.py` rules, `--commit`), which is the branch's diff as it
  landed, so the same mutants are asked about. A commit that changed no Rust
  is not dispatched: it would plan nothing.
- **The report goes to the merged pull request**: the run carries its
  number as the `pr` input, since a run on `main` has no branch to look the
  pull request up by.
- **At most [`slots.CAP`] at once**; the rest wait in the queue's own state
  ([`slots.FILE`], beside the hand-back memory), so a restart loses none and
  a full cap never drops one ([`slots.pump`]). Nothing is lost to GitHub's one
  pending run per concurrency group either: on `main` each run is its own group.

Losing nothing costs little: a scoped run takes up to 2 h 45, and the queue
usually merged the branch before its report came anyway (#912's comment on
the merged pull request). A failure here is a lost signal, never a stopped
queue: every call fails soft and is tried again on the next pass.
"""
