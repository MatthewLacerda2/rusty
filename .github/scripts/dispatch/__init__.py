"""The batch's dispatch steps, done by a script instead of by hand (#1569).

Two steps of a batch were rebuilt from prose by the orchestrator each time:
starting a cloud coder (a `RemoteTrigger` body assembled by hand) and choosing an
independent review's effort level (a rule about paths, applied by eye). Both are
mechanical, so they live here, one responsibility per module:

- [`brief`]: the newest `Brief` comment on an issue and its `Model:` line;
- [`routine`]: the one-off routine that starts a cloud coder on that brief;
- [`remote`]: the claude.ai routines API, reached through `claude mcp serve`;
- [`review_level`]: `high` or `medium` for a pull request, from its paths.

`coder.py` (`make coder ISSUE=N`) and `review-level.py` (`make review-level
PR=N`) are only the command lines.
"""
