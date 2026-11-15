"""What a merged pull request asked to have mutated ([`requested`], #1934)."""

from __future__ import annotations

import re

# The ask: a line of its own, read like the closing keywords are. The first one counts.
LINE = re.compile(r"^[ \t]*Mutation:[ \t]*(.*?)[ \t]*$", re.MULTILINE)

# A glob is plain path characters and git's glob ones; anything else (a sentence,
# a shell character) is unreadable, and unreadable asks for nothing. The value is
# passed as an argument, never through a shell, but the description is untrusted.
# It names a path (a `/`) or a Rust file, so a sentence after `Mutation:` is not globs.
GLOB = re.compile(r"[A-Za-z0-9_.*?{}\[\],-]*(/[A-Za-z0-9_./*?{}\[\],-]*|\.rs)")


def unlisted(word: str) -> list[str]:
    """`word` split at its commas outside braces: a coder lists paths as
    `a.rs, b.rs`, which the scope script's whitespace split read as a path ending
    in `,` (#2201), while a brace glob's commas (`src/{a,b}/**`) are its own."""
    parts, depth, start = [], 0, 0
    for i, ch in enumerate(word):
        depth += {"{": 1, "}": -1}.get(ch, 0)
        if ch == "," and depth == 0:
            parts.append(word[start:i])
            start = i + 1
    parts.append(word[start:])
    return [p for p in parts if p]


def requested(body: str | None) -> str | None:
    """`diff`, the space-separated globs, or `None` when nothing readable was asked."""
    found = LINE.search(body or "")
    if found is None:
        return None
    words = [w for word in found.group(1).replace("`", " ").split() for w in unlisted(word)]
    if not words:
        return None
    if words == ["diff"]:
        return "diff"
    return " ".join(words) if all(GLOB.fullmatch(w) for w in words) else None


def rust(files: list[str]) -> bool:
    """Whether any of a commit's files is Rust: `diff`'s rule, so nothing else plans a mutant."""
    return any(path.endswith(".rs") for path in files)


def inputs(scope: str, commit: str, number: int, request: str) -> dict[str, str]:
    """The workflow's inputs for a merged pull request's run on `main`."""
    return {"scope": scope, "commit": commit, "pr": str(number), "request": request}
