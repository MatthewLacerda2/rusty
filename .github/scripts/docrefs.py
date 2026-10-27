#!/usr/bin/env python3
"""`docrefs.py` — the paths, make verbs and namespaces the docs name exist (#973).

rusty's docs are what agents drive it by (CLAUDE.md, *Unity is the bar*), and
a path that no longer exists sends the next agent searching. Nothing noticed when a
move left one behind: the audit (#933) found ten in one pass. This scans every
tracked `*.md` outside fenced code blocks for inline code that names

- a repo path: a token starting `src/`, `tests/`, `tools/`, `docs/` or `.github/`
  must exist (a `:line` or `::symbol` suffix is dropped first);
- a make verb: `make <verb>` must be a Makefile target;
- CLAUDE.md's API namespace list: each must have its `docs/api/<Name>.md`.

Placeholders pass: a token holding `<`, `*`, `{`, `…` or `$` is a pattern, not a
path. A path in some other project (a game's `assets/…`) never matches a prefix.
"Exists" means tracked by git (a file, or a directory holding one), so a stale
local file can't hide a reference CI would miss. Anything else that merely looks
like a path (MCP's `tools/list`) takes an inline allow with its reason, on the
line or the one above: `<!-- docrefs: allow <why> -->`.
Exits 1 naming each stale reference as `file:line`.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PREFIXES = ("src/", "tests/", "tools/", "docs/", ".github/")
PLACEHOLDER = re.compile(r"[<*{…$]|\.\.\.")
CODE = re.compile(r"(?<!`)`([^`\n]+)`(?!`)")
ALLOW = re.compile(r"<!-- docrefs: allow \S.*-->")
NAMESPACES = re.compile(r"one stable set of namespaces \(([^)]*)\)", re.S)


def references(text: str) -> list[tuple[int, str]]:
    """(line, inline code) for every inline code span outside fenced blocks."""
    out, fenced = [], False
    for number, line in enumerate(text.splitlines(), 1):
        if line.lstrip().startswith(("```", "~~~")):
            fenced = not fenced
        elif not fenced:
            out.extend((number, m.group(1).strip()) for m in CODE.finditer(line))
    return out


def path_of(code: str) -> str | None:
    """The repo path `code` names, or None when it names none (or a pattern)."""
    token = code.split()[0] if code.split() else ""
    if not token.startswith(PREFIXES) or PLACEHOLDER.search(token):
        return None
    return re.split(r"::|:(?=\d)", token)[0].rstrip(".,;")


def verb_of(code: str) -> str | None:
    """The verb of a `make <verb> …` span, or None."""
    match = re.match(r"^make ([a-z][a-z-]*)\b", code)
    return match.group(1) if match else None


def stale(doc: str, text: str, exists, verbs: set[str]) -> list[str]:
    """`doc:line: …` for each path or verb `text` names that does not exist."""
    out, lines = [], text.splitlines()
    allowed = lambda n: any(ALLOW.search(l) for l in lines[max(n - 2, 0) : n])
    for line, code in references(text):
        if allowed(line):
            continue
        path, verb = path_of(code), verb_of(code)
        if path and not exists(path):
            out.append(f"{doc}:{line}: `{path}` does not exist")
        if verb and verb not in verbs:
            out.append(f"{doc}:{line}: `make {verb}` is not a Makefile target")
    return out


def missing_namespaces(claude_md: str, exists) -> list[str]:
    """CLAUDE.md's namespace list entries with no `docs/api/<Name>.md`."""
    match = NAMESPACES.search(claude_md)
    if not match:
        return ["CLAUDE.md: the API namespace list (\"one stable set of namespaces (…)\") is gone"]
    names = re.findall(r"`(\w+)`", match.group(1))
    return [f"CLAUDE.md: namespace `{n}` has no docs/api/{n}.md" for n in names
            if not exists(f"docs/api/{n}.md")]


def make_verbs(makefile: str) -> set[str]:
    return set(re.findall(r"^([a-z][a-z-]*):", makefile, flags=re.M))


def main() -> int:
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, check=True,
                             capture_output=True, text=True).stdout.split("\0")
    known = {p for p in tracked if p} | {str(d) for p in tracked for d in Path(p).parents}
    exists = lambda p: p.rstrip("/") in known
    verbs = make_verbs((ROOT / "Makefile").read_text())
    found = missing_namespaces((ROOT / "CLAUDE.md").read_text(), exists)
    for doc in sorted(p for p in tracked if p.endswith(".md")):
        found += stale(doc, (ROOT / doc).read_text(encoding="utf-8"), exists, verbs)
    print("\n".join(found) if found else "docrefs: ok")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
