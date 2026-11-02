"""Resolve a conflict inside a `sorted` list by itself, or say no (#1723).

`ci-merge` states the resolution as a rule: lists under a `sorted` marker
mostly merge clean, and when two entries do meet, keep both, in order. A
`// sorted: <why order is free>` marker (#911) is the author saying in writing
that no side matters there. Both sides' lists being sorted makes their union
sorted, and the `sorted` gate checks it wherever it runs (`lint.yml` skips it
on a Markdown-only pull request, and [`decide`] accepts a marked file outside
the rule's scan). So the queue applies the rule where, and only where, it holds. A file is
resolved when **every** hunk of it, read with `merge.conflictStyle=diff3`:

- lies inside one list governed by a `sorted` marker, the list's extent as the
  rule defines it ([`lists.governing`]), on both sides;
- only adds on both sides: every base entry is still there, unchanged, so nothing
  was edited or removed;
- adds entries, each one line at the list's depth with a key, plus any deeper
  continuation lines. A comment or attribute line anywhere in the hunk, or
  directly above it, is refused: `#[cfg(test)]` over a hunk adding `mod p;`
  would land on whichever entry sorts first, not the one it was written for;
- names no key twice with different text: the key skips keywords, so `mod b;`
  and `pub mod b;` are one key written two ways, and are refused (compared
  ignoring case, as the rule orders them); the same entry added by both counts once.

The hunk becomes the union of its entries, sorted by the rule's key. Everything
else is no: `Cargo.lock` (its procedure, `ci-merge` and #824, needs a compile), a
file with no `diff3` base section, a conflict git left without markers (a
delete, a rename). [`settle`] writes nothing unless every conflicted path
resolves, so a branch is never half-resolved by machine.

`src/app/registry.rs` stays out by construction: its order is the frame's system
schedule, so it carries no marker (a test holds that).
"""

from __future__ import annotations

import os

from merging.queue.conflict import lists

# Never resolved here: its own procedure needs a compile (`ci-merge`, #824).
REFUSED = ("Cargo.lock",)

OURS, BASE, THEIRS, END = "<<<<<<<", "|||||||", "=======", ">>>>>>>"

# The option `git merge-tree` runs under for the resolver, so each hunk shows
# its base (the take rewrites its conflicted files with `checkout --conflict=diff3`).
DIFF3 = ("-c", "merge.conflictStyle=diff3")


def hunks(lines: list[str]) -> list | None:
    """The file as plain lines and `(ours, base, theirs)` hunks; `None` when a
    hunk has no base section or its markers are malformed."""
    out, part, sides = [], None, None
    for line in lines:
        if part is None:
            if line.startswith(OURS):
                part, sides = 0, ([], [], [])
            else:
                out.append(line)
        elif part == 0 and line.startswith(BASE):
            part = 1
        elif part == 1 and line.rstrip("\r") == THEIRS:
            part = 2
        elif part == 2 and line.startswith(END):
            out.append(sides)
            part = None
        elif line.startswith((OURS, BASE, END)) or (part == 0 and line.rstrip("\r") == THEIRS):
            return None
        else:
            sides[part].append(line)
    return out if part is None else None


def view(parts: list, side: int) -> tuple[list[str], list[tuple[int, int]]]:
    """The file as one side wrote it, and where each hunk landed in it."""
    lines, spans = [], []
    for part in parts:
        if isinstance(part, tuple):
            spans.append((len(lines), len(lines) + len(part[side])))
            lines.extend(part[side])
        else:
            lines.append(part)
    return lines, spans


def entries(lines: list[str], depth: int):
    """`(continuation of the entry above, entries)`, each entry a tuple of lines;
    `None` when a line is no entry, no continuation, or a note."""
    lead, found = [], []
    for line in lines:
        if lists.is_note(line) or not line.strip() or lists.indent(line) < depth:
            return None
        if lists.indent(line) > depth:
            (found[-1] if found else lead).append(line)
        elif lists.key(line) is None:
            return None
        else:
            found.append([line])
    return tuple(lead), [tuple(e) for e in found]


def union(hunk: tuple, depth: int) -> list[str] | None:
    """The hunk's entries, both sides' additions kept, in key order; `None` if
    either side edited or removed anything, or a key repeats with new text."""
    read = [entries(side, depth) for side in hunk]
    if any(r is None for r in read):
        return None
    (ours_lead, ours), (base_lead, base), (theirs_lead, theirs) = read
    if not ours_lead == base_lead == theirs_lead:
        return None
    if any(e not in ours or e not in theirs for e in base):
        return None
    kept = list(dict.fromkeys([*base, *ours, *theirs]))
    keys = [lists.key(e[0]).lower() for e in kept]
    if len(set(keys)) != len(keys):
        return None
    kept.sort(key=lambda e: lists.key(e[0]).lower())
    return [*base_lead, *(line for e in kept for line in e)]


def resolved(text: str) -> str | None:
    """`text`, a conflicted file written with [`DIFF3`], resolved; `None` when
    any hunk fails the rule (see the module doc) or there is none."""
    parts = hunks(text.split("\n"))
    if parts is None or not any(isinstance(p, tuple) for p in parts):
        return None
    (ours, ours_spans), (theirs, theirs_spans) = view(parts, 0), view(parts, 2)
    merged, at = [], 0
    for part in parts:
        if not isinstance(part, tuple):
            merged.append(part)
            continue
        one, other = lists.governing(ours, *ours_spans[at]), lists.governing(theirs, *theirs_spans[at])
        at += 1
        if one is None or other is None or one[2] != other[2]:
            return None
        if noted_above(ours, ours_spans[at - 1][0]) or noted_above(theirs, theirs_spans[at - 1][0]):
            return None
        kept = union(part, one[2])
        if kept is None:
            return None
        merged.extend(kept)
    return "\n".join(merged)


def noted_above(lines: list[str], start: int) -> bool:
    """Whether the line directly above a hunk starting at `start` annotates the
    entry below it: a comment or attribute, but not the list's own marker."""
    return start > 0 and lists.is_note(lines[start - 1]) and lists.reason(lines[start - 1]) is None


def decide(path: str, text: str | None) -> str | None:
    """A conflicted file's resolution, `None` to hand it back."""
    if text is None or os.path.basename(path) in REFUSED:
        return None
    return resolved(text)


def settle(work: str, paths: list[str], unreadable: list[str] = ()) -> dict[str, bool]:
    """Whether each conflicted path in the worktree `work` resolves; when every
    one does, each is rewritten with its resolution (the caller stages them).
    A path in `unreadable` (git could not rewrite it with its base) does not."""
    made = {path: None if path in unreadable else decide(path, read(os.path.join(work, path))) for path in paths}
    if paths and all(text is not None for text in made.values()):
        for path, text in made.items():
            with open(os.path.join(work, path), "w", encoding="utf-8", newline="") as out:
                out.write(text)
    return {path: text is not None for path, text in made.items()}


def read(path: str) -> str | None:
    try:
        with open(path, encoding="utf-8", newline="") as text:
            return text.read()
    except (OSError, UnicodeDecodeError):
        return None
