"""The batch's dispatch steps, done by a script instead of by hand (#1569).

Two steps of a batch were rebuilt from prose by the orchestrator each time:
starting a cloud coder (a `RemoteTrigger` body assembled by hand) and choosing an
independent review's effort level (a rule about paths, applied by eye). Both are
mechanical, so they live here, one responsibility per module:

- [`brief`]: the newest `Brief` comment on an issue;
- [`routine`]: the one-off routine that starts a cloud coder on that brief;
- [`running`]: how many coders are running, against the limit of five (#1856);
- [`cap`]: a lower cap on them, set by `make coders --cap N` (#2630);
- [`model`]: which model a coder runs on, from the issue's labels or its `Model:` line (#2630);
- [`launch`]: creating a coder's routine, shared by `make coder` and the line;
- [`waiting`]: the coders waiting for a slot: the line, its order and its lock (#2382);
- [`drain`]: launching from the line, `make coders`' pass and `make coder`'s checks against it;
- [`remote`]: the claude.ai routines API, reached through `claude mcp serve`;
- [`shepherd`]: the shepherd's routine, one turn of a pull request's review loop, and its cap (#2629);
- [`stage`]: where that loop stands, and the record the shepherd posts (#2629);
- [`review_level`]: `high` or `medium` for a pull request, from its paths;
- [`rebriefs`]: how many briefs issues take, and why each re-brief happened (#1724);
- [`runlog`]: one coder session's run log, read for its times (#2452);
- [`cost`]: what each coder session cost in wall-clock, kept across runs (#2452).

`coder.py` (`make coder ISSUE=N`), `coders.py` (`make coders`), `shepherd.py` (`make shepherd PR=N`), `review-level.py` (`make review-level
PR=N`), `rebriefs.py` (`make rebriefs`) and `coder-cost.py` (`make coder-cost`) are only the command lines.
"""
