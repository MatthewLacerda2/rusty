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


def requested(body: str | None) -> str | None:
    """`diff`, the space-separated globs, or `None` when nothing readable was asked."""
    found = LINE.search(body or "")
    if found is None:
        return None
    words = found.group(1).replace("`", " ").split()
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
