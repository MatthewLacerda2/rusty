"""Every conflict the queue meets, one line each, and their totals (#1723).

Asked on 2026-11-02 how much conflict work a batch carries, nobody could say:
"conflict" appears in nearly every re-brief through a template line (59 of 61),
and a hand-back leaves no trace beyond the head [`watch.handbacks.MEMORY`]
remembers. Whether more conflict automation is worth building rests on that
count, so each distinct `(main, head)` pair that conflicts is written down once
([`note`]): when, which pull request, each conflicted path and whether the
sorted-list rule could resolve it ([`resolve`]), and how it ended. The watch's
pre-take check ([`watch.mainline.conflicts`]) polls the same pair again and
again, so a pair already written is not written twice.

The file is JSON lines, under the checkout's common git directory beside the
hand-back memory, so every worktree and every watch shares it. The watch prints
the totals for its own life in its closing summary; `make conflicts SINCE=12h`
prints them on demand.

## What the record answered (#1727)

The first batch it covered (2026-11-04 to 05) held 14 hand-backs, and the
sorted-list rule resolved none. Each was reproduced from its pull request's
merge commits and classed against #1727's rule (a check proves the result:
every line each side changed survives, nothing else moves). Three qualified,
all of them two sides editing adjacent but different lines (a doc line on one
side, a signature on the other), and a plain line-level merge reproduced the
coder's resolution exactly, so a model added nothing there. The other eleven
needed a choice or new prose: both sides rewording one doc sentence, a `mod`
line's visibility changed both ways, a refactor on one side meeting an edit on
the other. One "keep both" merge the check would have accepted (#1829's
`shader.wgsl`, where the coder moved `lighting_color += lightmap_specular`
elsewhere) was not what the code needed. So no model is called on a conflict;
read this record again before reopening the question.
"""

from __future__ import annotations

import json
import re
from datetime import datetime, timedelta, timezone
from pathlib import Path

from merging.queue import gitio

# The record, under the common git directory, beside [`watch.handbacks.MEMORY`].
FILE = "merge-queue/conflicts.jsonl"

# How a recorded conflict ended.
RESOLVED, HANDED_BACK = "resolved", "handed back"


def path(root: str) -> Path:
    """[`FILE`] in the checkout's common git directory, which every worktree shares."""
    common = gitio.git("rev-parse", "--git-common-dir", cwd=root).stdout.strip() or ".git"
    return Path(root, common, FILE)


def read(path: Path) -> list[dict]:
    """The recorded conflicts; a missing file is none, an unreadable line is skipped."""
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    found = []
    for line in text.splitlines():
        try:
            entry = json.loads(line)
        except ValueError:
            continue
        if isinstance(entry, dict):
            found.append(entry)
    return found


def note(path: Path, pull: int, head: str, main: str, paths: dict[str, bool], ended: str, now: datetime | None = None) -> bool:
    """Append one conflict unless its `(main, head)` pair is already recorded.
    `paths` says, per conflicted path, whether the rule resolves it. Returns
    whether a line was written; a file that cannot be written is no line, never
    a stop: the record is a count, not part of the merge."""
    if any(e.get("main") == main and e.get("head") == head for e in read(path)):
        return False
    when = (now or datetime.now(timezone.utc)).isoformat(timespec="seconds")
    line = {"at": when, "pull": pull, "main": main, "head": head, "paths": paths, "ended": ended}
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a") as out:
            out.write(json.dumps(line, sort_keys=True) + "\n")
    except OSError:
        return False
    return True


def since(entries: list[dict], when: datetime) -> list[dict]:
    """The entries recorded at or after `when`; one with no readable time is left out."""
    kept = []
    for entry in entries:
        try:
            at = datetime.fromisoformat(str(entry.get("at")))
        except ValueError:
            continue
        if at >= when:
            kept.append(entry)
    return kept


def totals(entries: list[dict]) -> list[str]:
    """What the entries add up to, as the summary's lines."""
    if not entries:
        return ["conflicts with `main`: none recorded."]
    resolved = sum(1 for e in entries if e.get("ended") == RESOLVED)
    pulls = len({e.get("pull") for e in entries})
    paths = [ok for e in entries for ok in (e.get("paths") or {}).values()]
    whole = sum(1 for e in entries if e.get("paths") and all(e["paths"].values()))
    return [
        f"conflicts with `main`: {len(entries)} on {pulls} pull request(s); {resolved} resolved"
        f" by the sorted-list rule, {len(entries) - resolved} handed back.",
        f"  {whole} of {len(entries)} had every conflicted file inside a `sorted` list;"
        f" {sum(paths)} of {len(paths)} conflicted file(s) were resolvable.",
    ]


def when(text: str, now: datetime) -> datetime:
    """SINCE as an aware time: an age (`12h`, `3d`) back from `now`, or ISO 8601
    (a date alone is its midnight UTC). Raises `ValueError` on anything else."""
    age = re.fullmatch(r"(\d+)([hd])", text)
    if age:
        return now - timedelta(**{"hours" if age.group(2) == "h" else "days": int(age.group(1))})
    at = datetime.fromisoformat(text.replace("Z", "+00:00"))
    return at if at.tzinfo else at.replace(tzinfo=timezone.utc)
