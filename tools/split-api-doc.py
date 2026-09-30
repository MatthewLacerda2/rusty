#!/usr/bin/env python3
"""Split `docs/scripting-api.md` into `docs/api/` (#569) — mechanically, so it re-runs.

Every `## ` section whose heading starts with a backticked namespace (`` ## `Physics` ``)
becomes `docs/api/<Namespace>.md`, verbatim. Everything else — the preamble and the
non-namespace sections (headless session, lifecycle callbacks, field schema) — stays
in `docs/api/index.md`, which gains a linked list of the namespaces in doc order.

The only rewrites are links, so they keep resolving from the new directory: a relative
link (`ui.md`) gains `../`, and an in-page anchor (`#physics`) that now lives in another
file points at that file. Fenced code is never touched. A trailing `---` rule at the end
of a section is dropped (the rule separated sections of one page, not the section).

Usage, from the repo root: `python3 tools/split-api-doc.py` — reads the single file,
rewrites `docs/api/` from scratch, and deletes the single file.
"""

import re
import sys
from pathlib import Path

SRC = Path("docs/scripting-api.md")
OUT = Path("docs/api")
INDEX = "index.md"
NAMESPACE = re.compile(r"^## `([A-Za-z_][A-Za-z0-9_]*)`")
HEADING = re.compile(r"^(#{1,6}) (.*)$")
LINK = re.compile(r"\]\(([^)\s]+)\)")


def sections(lines):
    """Yield (namespace or None, lines) chunks cut at every unfenced `## ` heading."""
    chunk, ns, fence = [], None, False
    for line in lines:
        if line.lstrip().startswith("```"):
            fence = not fence
        if not fence and line.startswith("## "):
            yield ns, chunk
            m = NAMESPACE.match(line)
            chunk, ns = [], (m.group(1) if m else None)
        chunk.append(line)
    yield ns, chunk


def trim(chunk):
    """Drop trailing blank lines and a trailing `---` rule."""
    while chunk and chunk[-1].strip() in ("", "---"):
        chunk.pop()
    return chunk


def slug(text):
    """GitHub's heading anchor: lowercase, punctuation dropped, spaces to hyphens."""
    text = re.sub(r"[^\w\- ]", "", text.strip().lower())
    return text.replace(" ", "-")


def anchors(files):
    """Map every heading anchor to the file it lands in (first wins, like GitHub)."""
    where = {"files": set(files)}  # the index's own list already points in-directory
    for name, chunk in files.items():
        fence = False
        for i, line in enumerate(chunk):
            if line.lstrip().startswith("```"):
                fence = not fence
            m = HEADING.match(line)
            if m and not fence:
                where.setdefault(slug(m.group(2)), (name, i == 0))
    return where


def relink(name, chunk, where):
    """Rewrite one file's links for its new home in `docs/api/`."""

    def fix(m):
        target = m.group(1)
        if target.startswith("#"):
            if target[1:] not in where:
                sys.exit(f"{name}: anchor {target} matches no heading")
            dest, top = where[target[1:]]
            if dest == name:
                return m.group(0)
            return f"]({dest})" if top else f"]({dest}{target})"
        if target in where["files"] or re.match(r"^[a-z]+:|^/", target):
            return m.group(0)
        return f"](../{target})"

    out, fence = [], False
    for line in chunk:
        if line.lstrip().startswith("```"):
            fence = not fence
        out.append(line if fence else LINK.sub(fix, line))
    return out


def main():
    lines = SRC.read_text().splitlines()
    head, tail, spaces = [], [], {}
    for ns, chunk in sections(lines):
        if ns is not None:
            spaces[f"{ns}.md"] = trim(chunk)
        else:
            (tail if spaces else head).append(trim(chunk))
    # The index keeps the doc's order: what came before the namespaces, the list of
    # links where the namespace sections were, then what came after them.
    listing = ["## Namespaces", "", "One file per namespace, in reference order:", ""]
    listing += [f"- [`{f[:-3]}`]({f})" for f in spaces]
    body = []
    for part in head + [listing] + tail:
        body += (["", "---", ""] if body else []) + part
    files = {INDEX: body, **spaces}

    where = anchors(files)
    if OUT.exists():
        for old in OUT.glob("*.md"):
            old.unlink()
    OUT.mkdir(parents=True, exist_ok=True)
    for name, chunk in files.items():
        (OUT / name).write_text("\n".join(relink(name, chunk, where)) + "\n")
    SRC.unlink()
    print(f"split {SRC} into {len(files)} files under {OUT}/")


if __name__ == "__main__":
    main()
