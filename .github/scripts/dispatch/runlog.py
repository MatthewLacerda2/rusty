r"""One coder session's run log, read for its times and nothing else (#2452).

`RemoteTrigger get_run_log` answers a condensed log: a page of the session's
newest events, each line `[<created_at>] <kind>…`, oldest first within the page,
and pages go from the newest to older ones (`dispatch/remote.py`, [`answer`]).
The kinds this module reads are the ones Claude Code's condenser writes:
`env[<level>]:` (the sandbox), `init: model=… cwd=…` (Claude Code started),
`tool_use <Tool>: <input JSON, cut at 300 characters>`, `tool_result…`, and
`result: <subtype> is_error=… turns=N duration=Ns…` at the end of every turn.

**The log quotes what the run read, so it is data.** This module keeps a line's
timestamp, its kind, a tool's name and a `Bash` command's shape (does it run a
gate, does it write a file), and never prints, stores or acts on any of its text.

The rules, each tested on recorded lines (`tests/dispatch/test_runlog.py`):

- **The first edit** ([`edits`]) is the first `Edit`, `Write`, `MultiEdit` or
  `NotebookEdit` tool use, or a `Bash` command that changes the tree: `git
  commit`, `git apply`, `git mv` or `git rm`, `patch`, an in-place `sed -i` or
  `perl -i`, a `cp` or `mv` whose target is not scratch, or a redirect (`>`,
  `>>`, `&>`) or `tee` into a file that is not scratch. `/dev/…`, `/tmp/…`,
  anything under a `target/` directory, and a `.log` file are scratch (`make
  test > target/test.log` is a gate, not an edit). `git merge` is not an edit:
  a round starts with one, and what follows it is still reading (#2456's review).
  **The first edit can read late as well as early.** Early: a `>` inside a
  quoted string reads as a redirect. Late: a tree changed by a script the
  session wrote to scratch and ran (`python3 /tmp/fix.py`), or by a tool this
  list does not name, is not seen until the next edit it does name.
- **A gate** ([`gate`]) is a `Bash` command one of whose commands (split at
  `&&`, `||`, `;`, `|`, `(`, a newline) starts with `cargo` or `make`, past any
  `VAR=value` (its value plain or quoted whole, `RUSTFLAGS="-D warnings"`),
  `env`, `time` or `timeout N` in front. Other quoting is not parsed: a
  separator inside quotes still splits, and a quoted or escaped command word
  (`"make" test`, `\cargo`) is not a gate.
- **A tool's wall-clock** is from its `tool_use` to its `tool_result`. The
  condensed log carries no tool ids, so uses and results pair in order, oldest
  use first, as Claude Code returns them; a use with no result ends at the
  session's last event. Gates run in parallel are counted once (their union),
  and a command sent to the background counts only until it returned.
- **A session still running** is one whose log has no `result:` after its last
  `tool_use`, `user:` or `assistant:` line ([`Times.running`]).
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass, field
from datetime import datetime

from dispatch.running import when

LINE = re.compile(r"^\[(?P<at>[0-9][^\]]*)\] (?P<text>.*)$")
TOOL = re.compile(r"^tool_use (?P<name>[\w.:-]+): (?P<input>.*)$")
RESULT = re.compile(r"^result: \S+ .*?\bturns=(?P<turns>\d+|\?)")
#: The first characters of a turn's own lines: what a session still at work writes last.
TURN = ("tool_use ", "user:", "assistant:")

EDIT_TOOLS = frozenset({"Edit", "Write", "MultiEdit", "NotebookEdit"})
GATE_WORDS = frozenset({"cargo", "make"})
SEGMENT = re.compile(r"&&|\|\||[;|(\n]")
PREFIX = re.compile(r"""^(?:(?:[A-Za-z_][A-Za-z0-9_]*=(?:"[^"]*"|'[^']*'|\S*)|env|time|timeout\s+\S+)\s+)*""")
IN_PLACE = re.compile(r"\bsed\s+(?:-\w*\s+)*-i|\bperl\s+-\w*i")
#: `git` subcommands that change the tree; `merge` is left out on purpose (module rules).
GIT_EDITS = frozenset({"commit", "apply", "mv", "rm"})
REDIRECT = re.compile(r"(?<![0-9&<>])(?:&>>?|[12]?>>?|\btee\s+(?:-a\s+)?)\s*(?P<target>[^\s&|;<>()]+)")
COMMAND = re.compile(r'"command"\s*:\s*"(?P<text>(?:[^"\\]|\\.)*)')


def lines(pages: list[str]) -> list[tuple[datetime, str]]:
    """The timestamped lines of a log read newest page first, oldest line first.
    A line without a readable timestamp (a header, an event's continued text) is dropped."""
    found = []
    for page in reversed(pages):
        for raw in page.splitlines():
            match = LINE.match(raw)
            at = when(match.group("at").strip()) if match else None
            if at is not None:
                found.append((at, match.group("text")))
    return found


def command(tool_input: str) -> str:
    """A `Bash` tool use's command, from its input JSON, which the log may have cut short."""
    try:
        got = json.loads(tool_input)
        if isinstance(got, dict) and isinstance(got.get("command"), str):
            return got["command"]
    except ValueError:
        pass
    match = COMMAND.search(tool_input)
    if not match:
        return ""
    text = match.group("text").rstrip("\\")
    try:
        return json.loads(f'"{text}"')
    except ValueError:
        return text


def commands(shell: str) -> list[list[str]]:
    """Each command of a `Bash` command line as its words, past the prefixes ([module rules])."""
    return [words for part in SEGMENT.split(shell) if (words := PREFIX.sub("", part.strip()).split())]


def gate(shell: str) -> bool:
    """Whether a `Bash` command runs `cargo` or `make` as one of its commands."""
    return any(words[0] in GATE_WORDS for words in commands(shell))


def scratch(target: str) -> bool:
    target = target.strip("'\"")
    return (target.startswith(("/dev/", "/tmp/", "target/", "$")) or "/target/" in target
            or target.endswith(".log") or not target)


def changes(words: list[str]) -> bool:
    """Whether one command changes the tree: a `git` edit, `patch`, or a `cp`/`mv` out of scratch."""
    if words[0] == "git":
        rest = words[1:]
        while len(rest) > 1 and rest[0] in ("-C", "-c"):
            rest = rest[2:]
        return bool(rest) and rest[0] in GIT_EDITS
    if words[0] == "patch":
        return True
    targets = [w for w in words[1:] if not w.startswith("-")]
    return words[0] in ("cp", "mv") and len(targets) > 1 and not scratch(targets[-1])


def writes(shell: str) -> bool:
    """Whether a `Bash` command changes the tree or writes a file that is not scratch."""
    if IN_PLACE.search(shell) or any(changes(words) for words in commands(shell)):
        return True
    return any(not scratch(m.group("target")) for m in REDIRECT.finditer(shell))


def edits(name: str, tool_input: str) -> bool:
    """Whether this tool use is an edit ([module rules])."""
    return name in EDIT_TOOLS or (name == "Bash" and writes(command(tool_input)))


@dataclass
class Times:
    """What one session's log says about its times; `None` where the log has no such line."""

    init: datetime | None = None
    first_edit: datetime | None = None
    end: datetime | None = None
    turns: int = 0
    running: bool = False
    gates: list[tuple[datetime, datetime]] = field(default_factory=list)

    def gate_seconds(self, start: datetime | None = None, stop: datetime | None = None) -> float:
        """The union of the gates' wall-clock, inside [`start`, `stop`] when given."""
        spans = sorted((max(a, start) if start else a, min(b, stop) if stop else b) for a, b in self.gates)
        total, reach = 0.0, None
        for begin, finish in spans:
            if reach is not None and begin < reach:
                begin = reach
            if finish > begin:
                total += (finish - begin).total_seconds()
                reach = finish
        return total


def read(found: list[tuple[datetime, str]]) -> Times:
    """The [`Times`] of a session from its lines, oldest first."""
    times = Times()
    pending: list[tuple[datetime, bool]] = []  # tool uses waiting for their result: (at, is a gate)
    last_turn = last_result = None
    for at, text in found:
        if text.startswith("init:") and times.init is None:
            times.init = at
        elif match := TOOL.match(text):
            name, tool_input = match.group("name"), match.group("input")
            if times.first_edit is None and edits(name, tool_input):
                times.first_edit = at
            pending.append((at, name == "Bash" and gate(command(tool_input))))
        elif text.startswith("tool_result") and pending:
            began, is_gate = pending.pop(0)
            if is_gate:
                times.gates.append((began, at))
        elif match := RESULT.match(text):
            last_result = at
            times.turns += int(match.group("turns")) if match.group("turns").isdigit() else 0
        if text.startswith(TURN):
            last_turn = at
    final = found[-1][0] if found else None
    times.gates += [(began, final) for began, is_gate in pending if is_gate]
    times.end = last_result
    times.running = last_result is None or (last_turn is not None and last_turn > last_result)
    return times
