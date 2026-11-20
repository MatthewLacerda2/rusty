"""`make bench-ab` and `make test-gpu PR=`: a pull request's measured rows as one verb (#2381).

Every performance PR owes a measured row before it queues, and on a desktop the
rows are only sound one way. The GPU rows, the memory rows and the op counters
repeat to under 1% between two takes; the CPU rows (`cpu_ms`, `renderer_ms`,
`render.<stage>`) drift 10 to 19% between two takes of the same binary minutes
apart (a `main` against `main` leg showed +15% on one scene during #2298's
sweep). So a claim is measured in three legs back to back, `main`, the PR's
head, `main` again, and the second `main` leg is each row's noise floor
(`legs`, `table`). Before this the protocol lived in scripts outside the
repository, and twice in one batch a measurement was wrong for a reason a verb
refuses (`refuse`): a baseline taken beside two local agents' builds, and a run
on a project whose textures were never imported.

- `trees`: `main` and the PR's head (with that `main` merged in), each checked
  out detached in a worktree of its own under `.claude/worktrees/`, built in its
  own `target/` and locked against `make reclaim` for the run; `bench-main` is
  kept between runs, the PR's tree removed once the PR is finished.
- `refuse`: what stops a timed run before it starts.
- `legs`: the three legs, each in a memory-capped scope where the machine has one.
- `stress`: `STRESS=1`'s three legs of the fixture's stress yard (`make bench`'s
  scenario), run on the same trees before the PR's is put back.
- `table`: the three legs' reports as the table a PR comment takes.
"""
