"""Whether pull request N has really been checked ([`decide.judge`]).

Exits 0 when CI genuinely ran on the head commit and passed, and non-zero with
a reason otherwise. It is the last thing the merge routine does before
`gh pr merge`, and it exists because "the checks look green" and "the checks
ran" are not the same claim. Ported from scorsese (#486), whose incidents the
reasoning below cites as scorsese#N.

**On rusty it is the only thing standing between a red pull request and
`main`.** `main` has no required status checks (#491 is the human task that
would add them), so GitHub will merge a pull request whose CI is red, running,
or absent. Every answer here that is not an explicit pass is a refusal, and an
absent check is never read as a green one.

## Two workflows, and why neither's green is the other's

A pull request here is checked by two workflows, `ci` (`ci.yml`) and `lint`
(`lint.yml`). Each collapses its gating jobs into one verdict job — `ci-gate`
and `lint-gate` — and **both must pass on the head commit**, because they
check different things: `ci` builds and tests, `lint` runs clippy, rustdoc and
the tools/lint gates. A green `ci` beside a missing `lint` is half a check.

**A gate job that passed is not yet evidence anything ran.** Both gates run
unless the run was cancelled (`!cancelled()`, #515) and pass when their gated jobs *succeeded or were skipped*. On a
draft every gated job skips by design — so a draft's run concludes success,
gate green, with nothing compiled. That is scorsese#153's shape: push to a
draft, mark it ready seconds later, and the `ready_for_review` run sometimes
never appears, leaving a ready pull request whose only run is the draft's. So a
run counts only when its gate passed **and** at least one gated job itself
concluded success ([`decide.ran_something`]). [`WORKFLOWS`] names the gated jobs, and a
test holds that list to each gate's `needs:` in the workflow files.

A Markdown-only pull request passes that test honestly: `build-test` and `lint`
still *run*, with their steps skipped, and report success (#218 is why the jobs
run rather than skip). Nothing compiled, by design, and the verdict says so.

**Except the scripting-API reference, `docs/api/`.** Two hard-gate tests parse it,
so CLAUDE.md treats a pull request touching it as code. CI's `code` filter does not (#525):
a pull request whose changes are all Markdown skips every step, drift tests
included. So when such a pull request touches that directory, the pass additionally
needs the `build-test` step that runs the dev-feature tests to have concluded
success ([`changes.drift_ran`]) — which today it cannot, and the refusal says why.
"""
