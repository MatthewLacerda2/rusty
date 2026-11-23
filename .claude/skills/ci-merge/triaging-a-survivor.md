# Triaging a survivor

Read the report when it lists survivors in code **this branch wrote**; others need
no reading. **Code behind GPU timestamps is never measured on CI**: lavapipe has no
`TIMESTAMP_QUERY`, so the `gpu_ms` / `gpu_passes` path and the bench's clock
normalisation run only on a real GPU and their mutants read `MISSED` everywhere.
Test its pure arithmetic with ordinary tests, re-measure the rest on the operator's
Mac (`make bench`, alternating before and after) when it changes, and don't file its
survivors as gaps. Everything else GPU is measured: the mutation workflows install
lavapipe and set `RUSTY_REQUIRE_GPU=1`, so a lost driver fails the run.

**A survivor is a question, not a defect**: is there behaviour here nothing asserts?

- **Test it** when it shows mechanism a game would notice wrong (a branch, a
  boundary, a sign, a formula, a state transition): through the callers' interface,
  named for the behaviour, beside its other tests, naming no mutant or line
  (`tool-named-test`). One test holding the class (`registry::default_drift`) beats
  a pin per instance. A real bug goes to the operator's list, as any finding.
- **Leave it**, no record owed, when a test could only restate the code: a constant
  or default, a message's wording, bench, fixture and diagnostic code, speed-only
  code, an equivalent mutant. Spend no run on equivalence unless it changes what
  gets written.
- **Exclude it at the source** when a kind keeps coming back: `.cargo/mutants.toml`,
  its reason beside it, narrow enough to swallow no real gap next door.

**No survivor count is a goal**: no issue, PR or brief asks for zero survivors or a
score. Stop the queue only for a report saying a **module** asserts nothing at all.

**Establish equivalence by applying the mutation and running the suite**, never by
reasoning about symmetry. Re-ask narrower (`make mutants-remote
SCOPE=crates/physics/src/query/through.rs`), or reproduce locally as the job runs it
(nextest, fail-fast, two test processes to fit the memory limit, and the workspace's
own baseline, because cargo-mutants' tests only the mutated package; `docs/testing.md`,
*Mutation testing*, says why):

    git diff origin/main > branch.diff
    cargo nextest run --no-run --workspace --features=dev   # build first, untimed
    cargo nextest run --workspace --features=dev --fail-fast --test-threads 2   # time it
    cargo mutants --workspace --test-tool nextest --baseline skip --timeout <3x secs> \
      --features dev --in-diff branch.diff -- --fail-fast --test-threads 2

`cargo mutants --list` names the functions and builds nothing: reach for it first.

Two shapes to recognise:

- **A measurement that discards sign cannot test an operation that changes it**:
  `Vec3::length`, a distance, a magnitude, a squared term, a count, an absolute
  value, a `Quat` dot are all blind to `+` → `-`.
- **A boundary comparison surviving both `<=` and `>=` means neither end is
  exercised**: an accumulator never landing exactly on a step, an agent exactly on
  its arrival radius, a contact at the epsilon, a trigger overlap beginning and
  ending on one tick. A real gap until measured otherwise.

Why this is not a gate: `--in-diff` line matching drifts after an update from
`main`, and a mutant inducing a slow loop times out and reports as a survivor.
