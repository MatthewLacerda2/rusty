"""The `sorted` rule's lists and keys, mirrored from `tools/lint/src/rules/sorted.rs` (#911, #1723).

The scripts are stdlib-only and run without a build (`.github/scripts/CLAUDE.md`),
so they cannot call `tools/lint`; this is the rule's reading of a list in
Python. Drift cannot land silently: the `sorted` gate runs in CI on whatever the
queue resolves, and `tools/lint/src/rules/sorted_keys.tsv` is one table of keys
that both this mirror's tests and the rule's own read. A change to the rule's
key, extent or note lines changes this module and that table in the same pull
request.
"""

from __future__ import annotations

# Words skipped before an entry's key starts (`KEYWORDS` in `sorted.rs`).
KEYWORDS = ("pub", "crate", "super", "self", "mod", "use", "fn")

# What the table writes for a line that is not an entry.
NOTE, NO_KEY = "<note>", "<none>"


def indent(line: str) -> int:
    return len(line) - len(line.lstrip())


def reason(line: str) -> str | None:
    """The reason a marker line gives, `None` when `line` is no marker."""
    text = line.strip()
    for prefix in ("// sorted:", "<!-- sorted:"):
        if text.startswith(prefix):
            rest = text[len(prefix):]
            while rest.endswith("-->"):
                rest = rest[:-3]
            return rest.strip()
    return None


def is_note(line: str) -> bool:
    """A comment or attribute line, which annotates an entry rather than being one."""
    return line.lstrip().startswith(("//", "#[", "<!--"))


def is_table_rule(line: str) -> bool:
    text = line.strip()
    return text.startswith("|") and "-" in text and all(c in "|-: " for c in text)


def _word(c: str) -> bool:
    return c.isalnum() or c == "_"


def key(line: str) -> str | None:
    """The line from its first non-keyword word on; `None` when it has no word."""
    rest = line
    while True:
        start = next((i for i, c in enumerate(rest) if _word(c)), None)
        if start is None:
            return None
        end = next((i for i in range(start, len(rest)) if not _word(rest[i])), len(rest))
        # `self-path` (a Markdown entry named for a rule) is a name, not the keyword.
        if rest[end:].startswith("-") or rest[start:end] not in KEYWORDS:
            return rest[start:].rstrip()
        rest = rest[end:]


def spelled(line: str) -> str:
    """A line's reading as the shared table writes it: its key, [`NOTE`] or [`NO_KEY`]."""
    if is_note(line):
        return NOTE
    found = key(line)
    return NO_KEY if found is None else found


def extent(lines: list[str], marker: int) -> tuple[int, int, int] | None:
    """The list a well-formed marker at `marker` governs: `(first, end, depth)`,
    the entries' line range and indent, or `None` when it governs none.

    From the line after the marker to the first blank line or the first line
    indented less than the first entry, past a Markdown table's header.
    """
    why = reason(lines[marker])
    if not why or (lines[marker].lstrip().startswith("<!--") and not lines[marker].rstrip().endswith("-->")):
        return None  # the rule calls this marker malformed and checks nothing under it
    end = marker + 1
    while end < len(lines) and lines[end].strip():
        end += 1
    first = marker + 1
    for at in range(marker + 1, end):
        if not lines[at].lstrip().startswith("|"):
            break
        if is_table_rule(lines[at]):
            first = at + 1
            break
    if first >= end:
        return None
    depth = indent(lines[first])
    if depth > indent(lines[marker]):
        return None
    last = first
    while last < end and indent(lines[last]) >= depth:
        last += 1
    return first, last, depth


def governing(lines: list[str], start: int, stop: int) -> tuple[int, int, int] | None:
    """The marked list whose entries hold every line in `[start, stop)`, if any.

    Walks up from `start` through non-blank lines to the nearest marker.
    """
    at = start - 1
    while at >= 0 and lines[at].strip():
        if reason(lines[at]) is not None:
            found = extent(lines, at)
            if found and found[0] <= start and stop <= found[1]:
                return found
            return None
        at -= 1
    return None
