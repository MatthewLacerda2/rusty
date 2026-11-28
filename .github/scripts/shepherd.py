#!/usr/bin/env python3
"""`shepherd.py N [--dry-run] [--stage] [--record FILE --verdict WORD --reviewed SHA [--brief URL]]` — the shepherd of PR #N (#2629).

`make shepherd PR=N` is this. Three uses, one reading of where the pull
request's review loop stands (`dispatch/stage.py`):

- **bare: start the shepherd**, a one-off cloud routine on the latest Opus
  whose session follows `.claude/agents/shepherd.md` for PR #N
  (`dispatch/shepherd.py`). It refuses (exit 1) when the loop has nothing to do
  (a stop: why is said) or a shepherd for #N is still in flight, and answers
  exit 3 when [`shepherd.CAP`] are in flight already: the quiet watch asks
  again on its next pass. The quiet watch runs it when a pull request turns
  ready with no standing verdict; the orchestrator or the operator may run it
  by hand, and the `queue` label and `Needs:` still override what it decides.
  Needs the `claude` CLI signed in to claude.ai (`dispatch/remote.py`), as
  `make coder` does, and `RUSTY_CLOUD_ENV` when no coder routine exists yet.
- **`--stage`: where the loop stands**, one line, `stage: review|reread|stop:
  <why> (rounds sent: K of 2); head <sha>`, the full head to review and to
  name in `--reviewed`; the shepherd's session runs it at its start and
  again before it posts, so two starts on one head never post two records.
- **`--record FILE --verdict queue|round|hold --reviewed SHA [--brief URL]`:
  the record to post**, printed: FILE's text, the `Round brief:` line for a
  round, and the verdict line on SHA, the head the session reviewed
  (`standing.line`), so the machine line is never typed by hand. **A verdict
  is given only on the head that was read** (the #2650 review): when the pull
  request's head is no longer SHA and commits of its own followed it (not
  merges of `main` alone, `standing.merges_only`, nor a compare GitHub did not
  show), it refuses, prints nothing, and says to review the new head; a push
  between the read and the post would otherwise get `Verdict: queue` unread.
  It refuses too (exit 1, nothing printed) when the loop stands at a stop,
  unless `--dry-run`, which prints it all the same and says the stage on
  stderr: the dry run on a merged pull request.

Exits 0 on a launch, a stage or a record, 1 on a refusal or a usage error, 2
when GitHub or claude.ai failed, 3 at the cap. REST only, so `--stage` and
`--record` run from the shepherd's cloud session.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone

import ghrest
from dispatch import brief, handed, launch, remote, stage
from dispatch import shepherd as routine
from dispatch.routine import ROUTINES_URL
from merging.queue import verdicts
from merging.verdict import standing

USAGE = ("usage: shepherd.py PR [--dry-run] [--stage]"
         " [--record FILE --verdict queue|round|hold --reviewed SHA [--brief URL]]")
FLAGS = ("--dry-run", "--stage")
VALUED = ("--record", "--verdict", "--reviewed", "--brief")
SHA = re.compile(r"[0-9a-f]{7,40}")


def say(message: str) -> None:
    print(f"shepherd: {message}", file=sys.stderr, flush=True)


def gh_json(path: str) -> object:
    done = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        return None
    try:
        return json.loads(done.stdout)
    except ValueError:
        return None


def parse(argv: list[str]) -> tuple[int, dict] | None:
    """The pull request's number and the options, or `None` for a usage error."""
    opts, rest, args = {}, [], list(argv)
    while args:
        arg = args.pop(0)
        if arg in VALUED:
            if not args:
                return None
            opts[arg] = args.pop(0)
        elif arg in FLAGS:
            opts[arg] = True
        else:
            rest.append(arg)
    if len(rest) != 1 or not rest[0].lstrip("#").isdigit() or ("--record" in opts) != ("--verdict" in opts):
        return None
    if ("--record" in opts) != ("--reviewed" in opts) or not SHA.fullmatch(opts.get("--reviewed", "0" * 7)):
        return None
    return int(rest[0].lstrip("#")), opts


def read(gh, repository: str, number: int):
    """The pull request (in the merge path's fields), its comments, where its loop stands,
    and `current(since)`: whether only merges followed `since` to its head (`None` unread)."""
    rest = gh(f"repos/{repository}/pulls/{number}")
    files = ghrest.pages(gh, f"repos/{repository}/pulls/{number}/files")
    comments = ghrest.pages(gh, f"repos/{repository}/issues/{number}/comments")
    if not isinstance(rest, dict) or files is None or comments is None:
        return None
    pull = ghrest.pull(rest, files)
    reader = verdicts.Reader(repository, gh, say=say)
    verdict, current = standing.said(comments), None
    if verdict:
        current = reader.current(verdict[1], pull["headRefOid"])
    return pull, comments, stage.where(pull, comments, current), lambda since: reader.current(since, pull["headRefOid"])


def told(found: stage.Stage) -> str:
    return f"stage: {found.what}: {found.why} (rounds sent: {found.rounds} of {stage.ROUNDS})"


def compose(pull: dict, found: stage.Stage, opts: dict, current) -> int:
    dry, reviewed, head = "--dry-run" in opts, opts["--reviewed"], pull["headRefOid"]
    if not current(reviewed):
        say(f"refused: #{pull['number']}'s head is {head[:7]} now, and commits of its own followed {reviewed[:7]},"
            " the head you reviewed (or GitHub did not show what followed it): nothing posted;"
            f" review {head[:7]}, then post its record")
        return 1
    if found.what == stage.STOP and not dry:
        say(f"refused: #{pull['number']} {told(found)}; nothing to post")
        return 1
    try:
        with open(opts["--record"], encoding="utf-8") as handle:
            text = handle.read()
        made = stage.record(text, opts["--verdict"], head if head.startswith(reviewed) else reviewed,
                            opts.get("--brief"))
    except (OSError, ValueError) as refused:
        say(f"refused: {refused}")
        return 1
    if dry:
        say(f"dry run: #{pull['number']} {told(found)}; the record it would post follows")
    print(made, end="")
    return 0


def start(number: int, pull: dict, found: stage.Stage, call, gh, repository: str, now: datetime,
          env: dict, dry: bool) -> int:
    if found.what == stage.STOP:
        say(f"refused: #{number} {told(found)}")
        return 1
    listed = launch.routines(call)
    flying = routine.in_flight(listed, lambda n: ghrest.pages(gh, f"repos/{repository}/issues/{n}/comments"), now)
    if number in flying:
        say(f"refused: #{number}'s shepherd {flying[number].get('name', '?')} is still in flight "
            f"(made {flying[number].get('created_at')}, no record since)")
        return 1
    if len(flying) >= routine.CAP:
        say(f"{len(flying)} shepherds in flight, the cap is {routine.CAP}: "
            f"{', '.join(f'#{n}' for n in sorted(flying))}; ask again when one posts its record")
        return 3
    made = routine.body(number, pull.get("title", ""), launch.environment_id(env, listed), repository, now)
    if dry:
        print(json.dumps(made, indent=2))
        say(f"dry run: nothing created; #{number} {told(found)}")
        return 0
    status, answer = call({"action": "create", "body": made})
    made_id = launch.trigger_id(answer)
    if status not in (200, 201) or not made_id:
        say(f"creating the routine answered HTTP {status}: {str(answer)[:500]}")
        return 2
    print(f"shepherd: #{number} ({found.what}) starts at {made['run_once_at']}: "
          f"{handed.link(made_id, ROUTINES_URL)}")
    return 0


def main(argv: list[str], gh=gh_json, call=remote.call, now: datetime | None = None,
         env: dict | None = None) -> int:
    parsed = parse(argv)
    if parsed is None:
        say(USAGE)
        return 1
    number, opts = parsed
    repository = ghrest.slug()
    got = read(gh, repository, number)
    if got is None:
        say(f"could not read #{number} or its comments from GitHub")
        return 2
    pull, _, found, current = got
    if "--stage" in opts:
        print(f"{told(found)}; head {pull['headRefOid']}")
        return 0
    if "--record" in opts:
        return compose(pull, found, opts, current)
    try:
        return start(number, pull, found, call, gh, repository, now or datetime.now(timezone.utc),
                     os.environ if env is None else env, "--dry-run" in opts)
    except brief.Refused as refused:
        say(f"refused: {refused}")
        return 1
    except remote.RemoteError as failed:
        say(str(failed))
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
