"""A reader of the nextest filtersets `shard_plan.filters` writes, so a test can
ask which shards take a test without nextest: `not`, `&`, `|`, parentheses,
`all()`, `none()`, `binary_id(=…)`, `test(=…)` and `test(/…/)`, with nextest's
precedence (`not` over `&` over `|`). Anything else raises, so a filter of
another shape fails the test instead of reading as a match."""

from __future__ import annotations

import re

TOKEN = re.compile(r"\s*(?:(?P<op>[()|&])|(?P<not>not\b)|(?P<fn>all\(\)|none\(\)"
                   r"|binary_id\(=[^)\s]+\)|test\(=[^)\s]+\)|test\(/(?:[^/\\]|\\.)*/\)))")


def tokens(text: str) -> list[str]:
    out, at = [], 0
    while at < len(text.rstrip()):
        m = TOKEN.match(text, at)
        if not m:
            raise ValueError(f"cannot read the filter at {text[at:at + 40]!r}")
        out.append(m.group(m.lastgroup))
        at = m.end()
    return out


def matches(text: str, binary: str, name: str) -> bool:
    """Whether the filterset `text` takes the test `name` of `binary`."""
    toks, at = tokens(text), 0

    def peek():
        return toks[at] if at < len(toks) else None

    def take():
        nonlocal at
        at += 1
        return toks[at - 1]

    def either() -> bool:
        got = both()
        while peek() == "|":
            take()
            got = both() or got
        return got

    def both() -> bool:
        got = one()
        while peek() == "&":
            take()
            got = one() and got
        return got

    def one() -> bool:
        tok = take()
        if tok == "not":
            return not one()
        if tok == "(":
            got = either()
            if take() != ")":
                raise ValueError("unbalanced parentheses")
            return got
        if tok in ("all()", "none()"):
            return tok == "all()"
        if tok.startswith("binary_id(="):
            return binary == tok[len("binary_id(="):-1]
        if tok.startswith("test(="):
            return name == tok[len("test(="):-1]
        if tok.startswith("test(/"):
            return re.search(tok[len("test(/"):-2], name) is not None
        raise ValueError(f"unexpected {tok!r}")

    got = either()
    if at != len(toks):
        raise ValueError(f"trailing {toks[at:]}")
    return got
