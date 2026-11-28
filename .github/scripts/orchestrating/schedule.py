"""The cloud orchestrator's routine: a fresh session every hour (#2667).

- **A schedule, a fresh session each fire**: a session's life is bounded, and
  GitHub holds the batch's state (`issue-batch`, *The orchestrator's loop*), so
  each fire picks the loop up from nothing. A fire that finds the lock held by
  a live orchestrator ends at once (`orchestrating/lease.py`).
- **Hourly** ([`CRON`]): the routines API refused a 30-minute schedule on
  2026-11-28 ("the minimum interval is 1 hour"); minute 0 is anchored by the
  server to the creation minute, so fires spread across the hour.
- **Made from this machine** (`make orchestrator`), through `claude mcp serve`
  (`dispatch/remote.py`): a routine made from a cloud session's own
  `create_trigger` starts its sessions with no repository (`dispatch/handed.py`).
- **Its prompt is a pointer** ([`PROMPT`]) at `.claude/agents/orchestrator.md`,
  never the procedure: the definition and the skill change without re-making it.
- **Found by its id first** ([`FILE`], on the lock, `orchestrating/carry.py`):
  the routines listing holds the newest 20, so a schedule older than that is
  missed; the id `make orchestrator` stored is asked for directly, and the
  listing is only the fallback, said with a warning when no id is stored.
- **Its tools add the `Claude_Code_Remote` ones it launches and lists with**
  ([`ALLOWED_TOOLS`]).
"""

from __future__ import annotations

import json
from datetime import datetime

from dispatch import routine
from dispatch.model import MODEL

NAME = "rusty orchestrator"
FILE = "orchestrator-schedule.json"  # on the lock: `{"id": "trig_…"}`
CRON = "0 * * * *"
PROMPT = "Read `.claude/agents/orchestrator.md` and follow it for one fire.\nRepository: {repository}."

REMOTE_TOOLS = [f"mcp__Claude_Code_Remote__{tool}" for tool in (
    "list_triggers", "list_sessions", "get_session", "create_session")]
ALLOWED_TOOLS = [*routine.ALLOWED_TOOLS, "Agent", *REMOTE_TOOLS]


def body(environment_id: str, repository: str, now: datetime, cron: str = CRON, model: str = MODEL) -> dict:
    """The `create` body of the hourly orchestrator routine."""
    made = routine.job(NAME, PROMPT.format(repository=repository), environment_id, repository, now,
                       model=model, tools=ALLOWED_TOOLS)
    del made["run_once_at"]
    made["cron_expression"] = cron
    return made


def ours(listed: list[dict]) -> list[dict]:
    """The enabled orchestrator routines among `listed`."""
    return [r for r in listed if r.get("name") == NAME and r.get("enabled")]


def stored(text: str | None) -> str | None:
    """The trigger id in [`FILE`]'s `text`, or `None` for none or one it cannot read."""
    try:
        found = json.loads(text or "null")
    except ValueError:
        return None
    made = found.get("id") if isinstance(found, dict) else None
    return made if isinstance(made, str) and made else None


def record(made_id: str) -> str:
    return json.dumps({"id": made_id}, indent=1, sort_keys=True) + "\n"
