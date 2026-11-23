# The batch's standing notes

The orchestrator keeps this file; the shepherd reads it at the start of every turn
(`shepherd.md`, step 1). A note here is a fact of the current batch that a review
needs and no other file carries: a sibling pull request in flight whose files this
one must stay out of, a decision the operator made that is not yet in an issue.

A rule that holds beyond the batch does not belong here: it goes into the root
`CLAUDE.md`, a nested one or a skill, where every agent reads it.

No notes yet.
