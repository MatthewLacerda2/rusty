# The comment-only check

A pull request whose diff changes only comments needs no `code-review` pass: nothing
it does can break the build or the behaviour. What it can break is the truth, since
a comment is what the next agent reads instead of the code. So the shepherd (or the
orchestrator) runs this check in place of the review.

1. **Prove it is comment-only**: on the pull request's head, detached, `make
   comment-only BASE=<its merge base with origin/main>` passes. If it fails, it is
   not this kind: review it as `shepherd.md`, step 3, says.
2. **Read every changed comment against the code it sits on**, and ask one
   question of each: does it now say what the code does? A doc comment that names a
   symbol, path or `make` verb that does not exist, a claim the code contradicts, or
   an absolute ("never", "always", "every") the code does not hold is a must-fix
   finding.
3. **The record** is `No review: comment-only (make comment-only passed on <sha>)`,
   then the findings, if any, and the verdict as `shepherd.md`, step 4, decides it.

Propose no rewording: a comment that is true and clumsy is not a finding.
