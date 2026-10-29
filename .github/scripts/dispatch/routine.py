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

import uuid
from datetime import datetime, timedelta, timezone

PROMPT = ("Read `.claude/skills/issue-batch/cloud-brief.md` and follow it.\n"
          "Issue(s): #{number}. Your brief is the comment at {brief}; read it there, not the newest `Brief`.")

ALLOWED_TOOLS = ["Bash", "Read", "Write", "Edit", "Glob", "Grep", "Skill", "TodoWrite", "ToolSearch", "WebFetch"]

#: Every coder routine's name starts with this, then the issue (`rusty #1569 …`).
NAME_PREFIX = "rusty #"
NAME_LENGTH = 60

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
    coders = [r for r in routines if str(r.get("name", "")).startswith(NAME_PREFIX)]
    for routine in sorted(coders, key=lambda r: r.get("created_at", ""), reverse=True):
        found = (((routine.get("job_config") or {}).get("ccr") or {}).get("environment_id"))
        if found:
            return found
    return None


def body(number: int, title: str, brief_url: str, model: str, environment_id: str, repository: str,
         now: datetime, event_id: str | None = None) -> dict:
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
