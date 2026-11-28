"""The routines API as a cloud session reaches it: through the session's own tools (#2667).

`dispatch/remote.py` reaches the API through `claude mcp serve`, whose
`RemoteTrigger` adds the claude.ai login's token. In a cloud session that
tool does not exist (`claude mcp serve` answered "Tool RemoteTrigger not
found" on 2026-11-28, and the API's endpoint answers 401 to a script), but the
session itself holds the `Claude_Code_Remote` tools. Proven that day:

- `create_trigger` makes a routine, but its fired session has **no repository**
  (its `session_context` carried no `sources`), and it takes no model or
  tools: unfit for a coder or a shepherd.
- `create_session` starts a session at once with the repository
  (`source_url`), the model and the tools: everything a coder's routine body
  sets. That is how a cloud orchestrator launches.

So when [`ENV`] names a file, `remote.call` answers from it instead
([`answer`]), and every script that launches (`make coder`, `make coders`,
`make shepherd`, the quiet watch's starts) runs unchanged:

- **`list`** answers the file's `triggers` (the session's `list_triggers`
  output) plus each of its `sessions` (its `list_sessions` output) titled
  `rusty …`, as a routine that fired when the session was made ([`as_routine`]),
  plus every create in the outbox. So a coder launched as a session holds its
  slot exactly as a fired routine does (`dispatch/running.py`), and a create
  earlier in the same pass is seen by the next re-list (`dispatch/drain.py`).
  A routine's own session is titled `⚡ rusty …`, so it is never counted twice.
- **`create`** puts the body in the file's `outbox` and answers 201. The
  session then sends each with `create_session`, its arguments exactly what
  [`session_args`] prints (`orchestrator.py outbox`), so no model writes a body.
- Anything else (`update`, `get_run_log`) answers 501: a cloud fire does not
  disable routines or read run logs.

The session copies only [`TRIGGER_FIELDS`] and [`SESSION_FIELDS`] of each row:
a routine's row is a few kilobytes, nearly all of it connectors.

The file's `environment_id` (the session's own, from `get_session`) stands in
for the environment the routines API would list, so `launch.environment_id`
finds it without `RUSTY_CLOUD_ENV`.
"""

from __future__ import annotations

import json
import os
from datetime import datetime, timezone
from pathlib import Path

ENV = "RUSTY_ROUTINES"

#: The titles of sessions the batch launched: coders (`rusty #`) and shepherds (`rusty shepherd #`).
TITLE_PREFIX = "rusty "


def path() -> Path | None:
    named = os.environ.get(ENV)
    return Path(named) if named else None


def load(file: Path) -> dict:
    try:
        got = json.loads(file.read_text())
    except (OSError, ValueError) as failed:
        raise OSError(f"{ENV}={file} cannot be read: {failed}") from failed
    if not isinstance(got, dict):
        raise OSError(f"{ENV}={file} holds no object")
    return got


def save(file: Path, data: dict) -> None:
    part = file.with_name(f".{file.name}.part")
    part.write_text(json.dumps(data, indent=1) + "\n")
    os.replace(part, file)


#: What the session copies of each row (the rest is never read): a routine's, then a session's.
TRIGGER_FIELDS = ("id", "name", "created_at", "enabled", "run_once_at", "last_fired_at", "ended_reason", "last_run")
SESSION_FIELDS = ("id", "title", "created_at")


def rows(found: object) -> list[dict]:
    """A listing as the tools answer it (`{"data": [...]}`, `list_sessions`' `{"ccr": {"data": [...]}}`)
    or a bare list."""
    if isinstance(found, dict):
        found = found.get("ccr", found)
        found = (found.get("data") or found.get("sessions") or []) if isinstance(found, dict) else []
    return [r for r in found if isinstance(r, dict)] if isinstance(found, list) else []


def placed(entry: dict, environment: str | None) -> dict:
    if environment:
        entry.setdefault("job_config", {"ccr": {"environment_id": environment}})
    return entry


def as_routine(session: dict, environment: str | None) -> dict | None:
    """A session the batch launched, as the fired routine `running` and `shepherd` read."""
    title = str(session.get("title") or "")
    made = session.get("created_at")
    if not title.startswith(TITLE_PREFIX) or not made:
        return None
    return placed({"id": session.get("id"), "name": title, "created_at": made, "enabled": False,
                   "last_fired_at": made, "run_once_at": made}, environment)


def pending(entry: dict, environment: str | None) -> dict:
    body = entry.get("body") or {}
    made = entry.get("created_at")
    listed = {"id": entry.get("id"), "name": body.get("name", ""), "created_at": made,
              "run_once_at": body.get("run_once_at"), "enabled": not entry.get("sent")}
    if entry.get("sent"):
        listed["last_fired_at"] = entry.get("sent")
    return placed(listed, environment)


def listing(data: dict) -> list[dict]:
    env = data.get("environment_id")
    found = [placed(dict(r), env) for r in rows(data.get("triggers"))]
    found += [r for s in rows(data.get("sessions")) if (r := as_routine(s, env))]
    return found + [pending(e, env) for e in data.get("outbox") or []]


def link(made_id: str, url: str) -> str:
    """Where a made routine is seen: its page, or, for one queued here, the command that sends it."""
    if made_id.startswith("outbox-"):
        return f"{made_id}, queued for `create_session` (`make orchestrator ARGS=outbox`)"
    return f"{url}{made_id} (its run links the session)"


def session_args(body: dict) -> dict:
    """The `create_session` arguments that start what the routine `body` would."""
    ccr = body["job_config"]["ccr"]
    context = ccr["session_context"]
    return {"title": body["name"], "prompt": ccr["events"][0]["data"]["message"]["content"],
            "model": context["model"], "source_url": context["sources"][0]["git_repository"]["url"],
            "extra_allowed_tools": context["allowed_tools"], "environment_id": ccr["environment_id"]}


def answer(arguments: dict, file: Path, now: datetime | None = None) -> tuple[int, object, str]:
    """`remote.answer`'s reply to `arguments`, from and into `file`."""
    data = load(file)
    action = arguments.get("action")
    if action == "list":
        return 200, {"data": listing(data)}, ""
    if action == "create":
        outbox = data.setdefault("outbox", [])
        made = {"id": f"outbox-{len(outbox) + 1}", "body": arguments["body"], "sent": None,
                "created_at": (now or datetime.now(timezone.utc)).strftime("%Y-%m-%dT%H:%M:%SZ")}
        outbox.append(made)
        save(file, data)
        return 201, {"id": made["id"]}, ""
    return 501, {"error": f"{action!r} is not answered from {ENV}: run it where `claude mcp serve` reaches the API"}, ""
