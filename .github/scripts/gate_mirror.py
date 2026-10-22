#!/usr/bin/env python3
"""Every `make gates` gate runs in CI with the same command (#964).

CLAUDE.md asks that every check run "in `make gates` and CI", and the Makefile's
`GATES` recipes are re-typed by hand as steps of `.github/workflows/{ci,lint}.yml`.
A gate that runs locally but not in CI is a false green on every pull request, so
`tests/test_gate_mirror.py` holds the two together through this module:

- each command a gate's recipe runs appears in some step of a gating workflow
  (`missing`), and
- every clippy run, local or CI, passes nothing after `--` but `-D warnings`
  (`clippy_extras`): the lint policy lives in `Cargo.toml`'s `[lints]`, and
  "CI adds only `-D warnings`" (`docs/linting.md`).

The comparison is one way: CI may run more than `make gates` (the macOS build,
the JUnit copies), never less. Stdlib only, a text parser for the narrow YAML
the workflows use; no build.
"""

from __future__ import annotations

import re
import shlex
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
WORKFLOWS = ("ci.yml", "lint.yml")

# Flags that change what a command prints, never what it checks. Each with its reason:
NEUTRAL = {
    # nextest's `ci` profile (.config/nextest.toml) only adds the JUnit report CI summarises.
    ("--profile", "ci"),
    # Output volume: the local gate is terse, CI's log is verbose for whoever reads it.
    ("--quiet",),
    ("--verbose",),
}


def makefile_gates(text: str) -> dict[str, list[str]]:
    """Each gate in `GATES` and the commands its recipe runs, variables expanded."""
    text = text.replace("\\\n", " ")
    variables = dict(re.findall(r"^([A-Z_]+) := (.*)$", text, flags=re.M))
    recipes: dict[str, list[str]] = {}
    target = None
    for line in text.splitlines():
        head = re.match(r"^([a-z-]+):", line)
        if head:
            target = head.group(1)
            recipes[target] = []
        elif line.startswith("\t") and target:
            recipes[target].append(line.strip())
        elif line and not line.startswith("#"):
            target = None
    expand = lambda cmd: re.sub(r"\$\((\w+)\)", lambda m: variables.get(m[1], m[0]), cmd)
    gates = {}
    for gate in expand(variables["GATES"]).split():
        commands = [expand(c.lstrip("@")) for c in recipes.get(gate, [])]
        # `command -v tool || { …install hint… }`: a local presence check; CI installs the tool.
        gates[gate] = [c for c in commands if not c.startswith("command -v ")]
    return gates


def gates_are_the_two_lists(text: str) -> bool:
    """`GATES` is defined as exactly `$(QUICK_GATES) $(HEAVY_GATES)` (#1179)."""
    return re.search(r"^GATES := \$\(QUICK_GATES\) \$\(HEAVY_GATES\)$", text, flags=re.M) is not None


def workflow_commands(text: str) -> list[str]:
    """Every shell command a workflow's steps run, a step's `env:` as a prefix.

    A `cargo-deny-action` step counts as `cargo deny <command>`.
    """
    commands = []
    for step in _steps(text):
        env = " ".join(f"{k}={shlex.quote(v)}" for k, v in step["env"].items())
        lines = step["run"]
        if "cargo-deny-action" in step.get("uses", ""):
            lines = [f"cargo deny {step['with'].get('command', '')}"]
        for line in lines:
            line = line.split("||")[0].strip()
            if line and not line.startswith("#"):
                commands.append(f"{env} {line}".strip())
    return commands


def _steps(text: str) -> list[dict]:
    steps, step, key, indent = [], None, None, 0
    for line in text.splitlines():
        depth = len(line) - len(line.lstrip())
        if step is not None and line.strip() and depth > indent and key:
            body = line.strip()
            if key == "run":
                step["run"].append(body)
            elif key in ("env", "with") and ":" in body:
                k, v = body.split(":", 1)
                step[key][k.strip()] = v.strip().strip("'\"")
            continue
        start = re.match(r"^(\s*)- (\w[\w-]*):\s*(.*)$", line)
        if start:
            step = {"run": [], "env": {}, "with": {}}
            steps.append(step)
            indent = len(start.group(1)) + 2
            line = " " * indent + f"{start.group(2)}: {start.group(3)}"
            depth = indent
        if step is None or depth != indent:
            step = step if depth > indent else None
            continue
        field = re.match(r"^\s*([\w-]+):\s*(.*)$", line)
        if not field:
            continue
        key, value = field.group(1), field.group(2).strip()
        if key == "run" and value and value != "|":
            step["run"].append(value)
        elif key == "uses":
            step["uses"] = value
    return steps


def normalise(command: str) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """A command as (its flags before `--`, unordered; what follows `--`, in order)."""
    try:
        tokens = shlex.split(command)
    except ValueError:  # a shell line shlex can't split (a heredoc, a quote across lines)
        tokens = command.split()
    before, after = (tokens, []) if "--" not in tokens else (
        tokens[: tokens.index("--")], tokens[tokens.index("--") + 1 :])
    for flag in NEUTRAL:
        for i in range(len(before) - len(flag) + 1):
            if tuple(before[i : i + len(flag)]) == flag:
                del before[i : i + len(flag)]
                break
    return tuple(sorted(before)), tuple(after)


def missing(gates: dict[str, list[str]], ci: list[str]) -> list[tuple[str, str]]:
    """(gate, command) for each gate command no CI step runs; a gate with none counts."""
    seen = {normalise(c) for c in ci}
    gaps = [(g, c) for g, cmds in gates.items() for c in cmds if normalise(c) not in seen]
    return gaps + [(g, "<no command>") for g, cmds in gates.items() if not cmds]


def clippy_extras(commands: list[str]) -> list[str]:
    """Clippy runs that pass anything after `--` but `-D warnings`."""
    clippy = [c for c in commands if re.search(r"\bcargo clippy\b", c)]
    return [c for c in clippy if normalise(c)[1] != ("-D", "warnings")]


def load() -> tuple[dict[str, list[str]], list[str]]:
    """Today's gates and the gating workflows' commands."""
    gates = makefile_gates((ROOT / "Makefile").read_text())
    ci = [c for w in WORKFLOWS for c in workflow_commands((ROOT / ".github/workflows" / w).read_text())]
    return gates, ci
