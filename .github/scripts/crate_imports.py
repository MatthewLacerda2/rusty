"""How a moved module's imports of a sibling are rewritten, for `make move-crate`
(`move_crate.py`): the plain forms only, and a list of the rest.

Inside the crate `crate::<dep>::` becomes `rusty_<dep>::`, which settles every path and
every group of paths. What it cannot settle is an import that *binds the module's name*,
so that the file goes on writing `<dep>::`: `use crate::<dep>;`, and a group's top-level
`self`. The plain forms of those are rewritten:

- `use crate::<dep>;` (any attributes above it go with it) and `use crate::<dep>::{self};`:
  the import goes, and each `<dep>::` names `rusty_<dep>::`;
- `use crate::<dep>::{self, A, B};` on one line, flat, without a comment, `self` first:
  `self, ` goes, and each `<dep>::` names the crate;
- a re-export keeps its name: `pub use crate::<dep>;` → `pub use rusty_<dep> as <dep>;`,
  `pub use crate::<dep>::{self, A};` → `{self as <dep>, A}`;
- `use crate::<dep> as x;` → `use rusty_<dep> as x;`.

Every other form a `self` sits in (nested beside other groups, not first, a comment in
the group, `{self,}`, a group over several lines), and any `crate::<dep>` the rules left,
is **listed** for the by-hand pass, never guessed: four reviews in a row each found one
more such form that a regex rewrite got silently wrong (PRs #2149, #2156, #2157, #2159).
A listed form the compiler would reject is fine left to the compiler.
"""

from __future__ import annotations

import re

VIS = r"(?:pub(?:\([\w: ]+\))?\s+)?"


def _group_end(text: str, at: int) -> int:
    """The index of the `}` closing the `{` at `at`; `len(text)` if none."""
    depth = 0
    for n in range(at, len(text)):
        depth += {"{": 1, "}": -1}.get(text[n], 0)
        if depth == 0:
            return n
    return len(text)


def _entries(body: str) -> list[str]:
    """A group's top-level entries, stripped (an empty one for a trailing comma)."""
    out, depth, cur = [], 0, ""
    for c in body:
        depth += {"{": 1, "}": -1}.get(c, 0)
        if c == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += c
    return [*out, cur.strip()]


def _groups(text: str, dep: str):
    """Each `[pub] use rusty_<dep>::{…}` group with a bare top-level `self`: (match, end, body, plain)."""
    for m in re.finditer(rf"\b({VIS})use rusty_{dep}::\{{", text):
        end = _group_end(text, m.end() - 1)
        body = text[m.end():end]
        entries = _entries(body)
        if "self" not in entries:
            continue
        plain = ("\n" not in body and "{" not in body and "//" not in body and "/*" not in body
                 and entries[0] == "self" and entries.count("self") == 1 and "" not in entries[1:])
        yield m, end, body, plain


def siblings(text: str, deps: set[str]) -> tuple[str, set[str]]:
    """`text` with `crate::<dep>::` → `rusty_<dep>::` and the plain import forms settled; the deps named."""
    named = set()

    def sibling(m: re.Match) -> str:
        if m.group(1) not in deps:
            return m.group(0)
        named.add(m.group(1))
        return f"rusty_{m.group(1)}::"

    # Siblings before the module's own: `crate::<m>::scene::` must not read as the sibling `scene`.
    text = re.sub(r"(?<![\w$])crate::(\w+)::", sibling, text)
    # A macro's `$crate` is now this crate, so its `$crate::<dep>::` names the sibling from the root.
    text = re.sub(r"\$crate::(\w+)::", lambda m: f"::{sibling(m)}" if m.group(1) in deps else m.group(0), text)
    for dep in sorted(deps):
        text, bare = _bind(text, dep)
        if bare:
            named.add(dep)
            text = re.sub(rf"(?<![\w:$]){dep}::", f"rusty_{dep}::", text)
    return text, named


def _bind(text: str, dep: str) -> tuple[str, bool]:
    """`text` with `dep`'s plain name-binding imports settled; whether a private one went."""
    text = re.sub(rf"\b({VIS}use )crate::{dep} as (\w+);", rf"\1rusty_{dep} as \2;", text)
    text = re.sub(rf"\b(pub(?:\([\w: ]+\))?\s+use )crate::{dep};", rf"\1rusty_{dep} as {dep};", text)
    attrs = r"^(?:[ \t]*#\[[^\n]*\]\n)*[ \t]*"
    lone = re.compile(rf"{attrs}use (?:crate::{dep}|rusty_{dep}::\{{\s*self\s*\}});\n", re.M)
    bare = bool(lone.search(text))
    text = lone.sub("", text)
    for m, end, body, plain in reversed(list(_groups(text, dep))):
        if not plain:
            continue
        head = text[:m.end()]
        if m.group(1):
            text = head + body.replace("self", f"self as {dep}", 1) + text[end:]
        else:
            text = head + re.sub(r"^\s*self\s*,\s*", "", body) + text[end:]
            bare = True
    return text, bare


def unsettled(path: str, text: str, deps: set[str]) -> list[str]:
    """`path:line: why` for each sibling import the plain rules did not settle, in the rewritten `text`."""
    out = []
    for dep in sorted(deps):
        for m, *_ in _groups(text, dep):
            out.append(f"{path}:{text.count(chr(10), 0, m.start()) + 1}: a `rusty_{dep}::{{…}}` group binds "
                       f"`self` in a form the rewrite leaves alone: settle `{dep}::` by hand")
        for m in re.finditer(rf"(?<![\w$])crate::{dep}\b", text):
            out.append(f"{path}:{text.count(chr(10), 0, m.start()) + 1}: `crate::{dep}` the rewrite did not "
                       f"settle: name `rusty_{dep}` by hand")
    return out
