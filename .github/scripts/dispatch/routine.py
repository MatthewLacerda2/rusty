"""The one-off routine that starts a cloud coder on its brief (#1569).

What `issue-batch`, *Launching a cloud coder*, had the orchestrator assemble by
hand for every coder, built the same way every time:

- **The prompt is a pointer at one comment** ([`PROMPT`]). The brief is a
  comment on the issue (#910), and the routine never carries its text (it is
  echoed back about three times). It names the URL of the exact comment
  `brief.read` checked, not "the newest `Brief`": the repository is public, the
  routine fires minutes after launch (28 late on 2026-10-29), and in that window
  anyone could post a newer comment starting with `Brief` that the coder would
  follow with push access (PR #1572's review).
- **`run_once_at` is computed**, a minute or two ahead: the API refuses a time
  already past, and `date -d` vs `date -v` differed between Linux and macOS.
- **The environment is `anthropic_cloud`**, never a `bridge` one (that is this
  machine; #504). The routines API cannot list environments, so its id comes
  from `RUSTY_CLOUD_ENV`, or else from the newest coder routine already made
  (every one of them was made in it, by this rule): see [`environment`].
- **`allowed_tools` includes `ToolSearch`**: the GitHub MCP tools load through
  it without being listed.

A re-brief is the same launch again: a new routine whose prompt names the new
`Brief`. Nothing has to remember the first routine's id.
"""

from __future__ import annotations

import re
import uuid
from datetime import datetime, timedelta, timezone

PROMPT = ("Read `.claude/skills/issue-batch/cloud-brief.md` and follow it.\n"
          "Issue(s): #{number}. Your brief is the comment at {brief}; read it there, not the newest `Brief`.")

ALLOWED_TOOLS = ["Bash", "Read", "Write", "Edit", "Glob", "Grep", "Skill", "TodoWrite", "ToolSearch", "WebFetch"]

#: Every coder routine's name starts with this, then the issue (`rusty #1569 …`).
NAME_PREFIX = "rusty #"
NAME_LENGTH = 60

#: Whose routines show the cloud environment. The API lists only the account's 20
#: newest and offers no paging that works, so on 2026-11-02 scorsese's filled the
#: list and `make coder` refused here. Both projects use one `anthropic_cloud`
#: environment, and both name a routine this way, so either answers.
ENVIRONMENT_FROM = (NAME_PREFIX, "scorsese #")

#: How far ahead the routine fires; the API refuses a time already past.
LEAD = timedelta(minutes=2)

ROUTINES_URL = "https://claude.ai/code/routines/"


def fire_time(now: datetime) -> str:
    """`now` plus [`LEAD`], down to the minute, as the RFC 3339 UTC the API takes."""
    at = (now.astimezone(timezone.utc) + LEAD).replace(second=0, microsecond=0)
    return at.strftime("%Y-%m-%dT%H:%M:00Z")


def name(number: int, title: str) -> str:
    """`rusty #N <title>`, cut at a word to [`NAME_LENGTH`]."""
    full = f"{NAME_PREFIX}{number} {' '.join(title.split())}"
    if len(full) <= NAME_LENGTH:
        return full
    return full[:NAME_LENGTH].rsplit(" ", 1)[0]


def environment(routines: list[dict]) -> str | None:
    """The environment of the newest coder routine, or `None` when there is none."""
    coders = [r for r in routines if str(r.get("name", "")).startswith(ENVIRONMENT_FROM)]
    for routine in sorted(coders, key=lambda r: r.get("created_at", ""), reverse=True):
        found = (((routine.get("job_config") or {}).get("ccr") or {}).get("environment_id"))
        if found:
            return found
    return None


#: The model a coder runs on when its issue names none (#1726). A new generation
#: changes this line and [`MODELS`], nothing else.
MODEL = "claude-opus-5-5"

#: What an issue's `Model:` line may ask for (#2490): the operator's decision for one
#: job, written on the issue; an orchestrator never adds the line on its own.
MODELS = {"opus": MODEL, "sonnet": "claude-sonnet-5-5"}

#: A line of its own in the issue body, `Model: <name>`, ignoring case and spaces.
MODEL_LINE = re.compile(r"[ \t]*Model:[ \t]*(\S*)[ \t]*", re.IGNORECASE)
FENCE = re.compile(r"[ \t]*(```|~~~)")


def model_lines(issue_body: str) -> list[str]:
    """The word of each whole `Model: <word>` line outside a fenced block; a line
    with more after the word is prose, not the line."""
    asked, fenced = [], False
    for line in issue_body.splitlines():
        if FENCE.match(line):
            fenced = not fenced
        elif not fenced and (found := MODEL_LINE.fullmatch(line)):
            asked.append(found.group(1))
    return asked


def model_for(issue_body: str | None) -> str:
    """The model id the issue's `Model:` line asks for, or [`MODEL`] when it has none.

    Raises `ValueError` naming the accepted values for a line that names no model
    in [`MODELS`], or for two lines: a guess would launch the wrong coder."""
    asked = model_lines(issue_body or "")
    if not asked:
        return MODEL
    accepted = ", ".join(sorted(MODELS))
    if len(asked) > 1:
        raise ValueError(f"the issue has {len(asked)} `Model:` lines; keep one, naming one of {accepted}")
    name = asked[0].strip().lower()
    if name not in MODELS:
        raise ValueError(f"the issue's `Model: {asked[0].strip()}` names no model; write one of {accepted}, "
                         "or drop the line for the default")
    return MODELS[name]


def body(number: int, title: str, brief_url: str, environment_id: str, repository: str,
         now: datetime, event_id: str | None = None, model: str = MODEL) -> dict:
    """The `create` body for the coder of issue `number`, on `model`, pinned to `brief_url`."""
    if not brief_url.startswith(f"https://github.com/{repository}/issues/{number}#issuecomment-"):
        raise ValueError(f"{brief_url!r} is not a comment on #{number}")
    prompt = PROMPT.format(number=number, brief=brief_url)
    return {
        "name": name(number, title),
        "run_once_at": fire_time(now),
        "enabled": True,
        "job_config": {"ccr": {
            "environment_id": environment_id,
            "session_context": {
                "model": model,
                "sources": [{"git_repository": {"url": f"https://github.com/{repository}"}}],
                "allowed_tools": ALLOWED_TOOLS,
            },
            "events": [{"data": {
                "uuid": event_id or str(uuid.uuid4()),
                "session_id": "",
                "type": "user",
                "parent_tool_use_id": None,
                "message": {"content": prompt, "role": "user"},
            }}],
        }},
    }
