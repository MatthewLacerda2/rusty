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

- `GATES` is exactly `$(QUICK_GATES) $(HEAVY_GATES)` (`gates_are_the_two_lists`),
  and the cloud brief's heavy steps name every `HEAVY_GATES` entry
  (`brief_misses_heavy`): a hand-copied gate list had drifted and `docrefs` went
  unrun before CI (#1179, PR #1177).

- a suite CI splits across a matrix runs whole (`shard_gaps`, #2264):
  `normalise` drops `--partition <value>` ([`NEUTRAL_WITH_VALUE`]) so a shard's
  command matches its gate, and in exchange every split lists each shard of
  `1..n` once, and every `--partition` reads `count:${{ matrix.shard }}` in a
  job whose one-line `- { …, shard: k/n }` entries it read complete. The macOS
  leg's split by `main`'s times (#2397) is held the same way: its
  `shard_plan.py args` must read `"${{ matrix.shard }}"` in such a job, and the
  script's filters take every test once whatever the plan holds
  (`tests/shard_plan/`). A split whose matrix it cannot read is flagged, never
  passed.

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
#: A flag dropped with whatever value follows it. `--partition count:k/n`: CI
#: splits one suite across a matrix's shards (#2264), and `test_shards` holds
#: that every shard of each split is an entry, so together they run it whole.
NEUTRAL_WITH_VALUE = ("--partition",)


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
        # `cmd || { …failure hint… }`: the hint is what a red gate prints, not
        # what it checks. Only a trailing brace group goes: `a || rc=1; b || rc=1`
        # keeps both commands, so CI dropping `b` is still flagged.
        commands = [re.sub(r"\s*\|\|\s*\{.*\}\s*$", "", expand(c.lstrip("@"))).strip()
                    for c in recipes.get(gate, [])]
        commands = [part for c in commands for part in each_command(c)]
        # `command -v tool || { …install hint… }`: a local presence check; CI installs the tool.
        gates[gate] = [c for c in commands if not c.startswith("command -v ")]
    return gates


#: What a run-on recipe does besides its commands: the flag it keeps, a header it
#: prints, and the verdict at the end.
BOOKKEEPING = re.compile(r"^(rc=0|failed=.*|echo .*|exit .*|if .*|then .*|fi)$")
#: How a run-on command notes its failure, then goes on.
NOTE_FAILURE = re.compile(r'\s*\|\|\s*(rc=1|failed="[^"]*")$')


def each_command(line: str) -> list[str]:
    """`rc=0; a || rc=1; b || rc=1; exit $$rc`, a recipe that runs every command and
    fails after if any did (or notes each failure in `failed=`, its headers echoed),
    as its commands `a` and `b`; any other line as itself."""
    if not re.search(r"\|\|\s*(rc=1|failed=)", line):
        return [line]
    parts = (NOTE_FAILURE.sub("", p.strip()) for p in line.split(";"))
    return [p for p in parts if p and not BOOKKEEPING.match(p)]


def gates_are_the_two_lists(text: str) -> bool:
    """`GATES` is defined as exactly `$(QUICK_GATES) $(HEAVY_GATES)` (#1179)."""
    variables = dict(re.findall(r"^([A-Z_]+) := (.*)$", text, flags=re.M))
    return variables.get("GATES", "").split() == ["$(QUICK_GATES)", "$(HEAVY_GATES)"]


def brief_misses_heavy(makefile: str, brief: str) -> list[str]:
    """`HEAVY_GATES` entries the brief's *The container* steps never mention (#1179)."""
    heavy = dict(re.findall(r"^([A-Z_]+) := (.*)$", makefile, flags=re.M))["HEAVY_GATES"]
    return [g for g in heavy.split() if not re.search(rf"`make [^`]*\b{g}\b|make {g}\b", brief)]


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
    for flag in NEUTRAL_WITH_VALUE:
        while flag in before[:-1]:
            at = before.index(flag)
            del before[at : at + 2]
    for flag in NEUTRAL:
        for i in range(len(before) - len(flag) + 1):
            if tuple(before[i : i + len(flag)]) == flag:
                del before[i : i + len(flag)]
                break
    return tuple(sorted(before)), tuple(after)


def missing(gates: dict[str, list[str]], ci: list[str]) -> list[tuple[str, str]]:
    """(gate, command) for each gate command no CI step runs; a gate with none counts.

    A step running `make <gate>` runs that gate's recipe itself, so it covers
    every command in it (`fuzz-check`, whose recipe is one script)."""
    seen = {normalise(c) for c in ci}
    by_make = {g for g in gates if normalise(f"make {g}") in seen}
    gaps = [(g, c) for g, cmds in gates.items() if g not in by_make
            for c in cmds if normalise(c) not in seen]
    return gaps + [(g, "<no command>") for g, cmds in gates.items() if not cmds]


#: A matrix entry that names a shard, written as a flow mapping on one line.
SHARD_ENTRY = re.compile(r"^\s*- \{(?P<body>[^}]*\bshard: (?P<k>\d+)/(?P<n>\d+)\b[^}]*)\}\s*$", re.M)
#: A job's key under `jobs:` (two spaces in, nothing after the colon).
JOB = re.compile(r"^  (?P<name>[\w-]+):\s*$", re.M)
#: The one `--partition` value that follows the matrix: each entry runs its shard.
SHARDED = ("count:${{ matrix.shard }}", '"count:${{ matrix.shard }}"', "'count:${{ matrix.shard }}'")
#: The time-weighted split's call (`shard_plan.py`) and the one shard argument
#: that follows the matrix.
PLANNED = re.compile(r"shard_plan\.py args\b[ ]?")
PLANNED_SHARD = ('"${{ matrix.shard }}"', "'${{ matrix.shard }}'", "${{ matrix.shard }}")


def _jobs(text: str) -> list[tuple[str, str]]:
    """Each job's name and body, `#` comment lines dropped."""
    code = "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))
    marks = list(JOB.finditer(code))
    return [(m["name"], code[m.end(): nxt.start() if nxt else len(code)])
            for m, nxt in zip(marks, marks[1:] + [None])]


def _split_gaps(job: str, body: str) -> tuple[bool, list[str]]:
    """Whether the job's matrix names shards, and each split that leaves one out:
    a split (the entry's other keys bar the per-entry flags) runs every `k` of
    `1..n` once."""
    splits: dict[tuple[str, int], list[int]] = {}
    for m in SHARD_ENTRY.finditer(body):
        keys = [kv.strip() for kv in m["body"].split(",") if kv.strip() and not kv.strip().startswith("shard:")]
        split = (", ".join(k for k in keys if not k.endswith((": true", ": false"))), int(m["n"]))
        splits.setdefault(split, []).append(int(m["k"]))
    return bool(splits), [f"{job}: {name or 'shards'} of {n} has {sorted(ks)}; list each shard of 1..{n} once"
                          for (name, n), ks in splits.items() if sorted(ks) != list(range(1, n + 1))]


def shard_gaps(text: str) -> list[str]:
    """What keeps a workflow's split suites from running whole (#2264). `missing`
    drops `--partition`, so this holds instead that each split lists every shard
    once, and that every `--partition` (and `shard_plan.py args`, #2397) sits
    in a job whose split it saw complete and reads `count:${{ matrix.shard }}`
    (`"${{ matrix.shard }}"`). A split it cannot read (a block-YAML
    matrix) is a gap, not a pass: it cannot vouch for what it did not see."""
    gaps: list[str] = []
    for job, body in _jobs(text):
        found, bad = _split_gaps(job, body)
        gaps += bad
        for m in PLANNED.finditer(body):
            value = body[m.end():].split("\n", 1)[0].split(")", 1)[0].strip()
            if not found:
                gaps.append(f"{job}: `shard_plan.py args` with no matrix split found; write each entry on one line "
                            f"as `- {{ os: …, shard: k/n }}`")
            elif not bad and value not in PLANNED_SHARD:
                gaps.append(f"{job}: `shard_plan.py args {value}` does not follow the matrix; "
                            f"write `shard_plan.py args \"${{{{ matrix.shard }}}}\"`")
        for m in re.finditer(r"--partition\b[ =]?", body):
            value = body[m.end():].split("\n", 1)[0].split(" ||", 1)[0].strip()
            if not found:
                gaps.append(f"{job}: `--partition` with no matrix split found; write each entry on one line "
                            f"as `- {{ features: …, shard: k/n }}`")
            elif not bad and not value.startswith(SHARDED):
                gaps.append(f"{job}: `--partition {value}` does not follow the matrix; "
                            f"write `--partition \"count:${{{{ matrix.shard }}}}\"`")
    return gaps


def clippy_extras(commands: list[str]) -> list[str]:
    """Clippy runs that pass anything after `--` but `-D warnings`."""
    clippy = [c for c in commands if re.search(r"\bcargo clippy\b", c)]
    return [c for c in clippy if normalise(c)[1] != ("-D", "warnings")]


def load() -> tuple[dict[str, list[str]], list[str]]:
    """Today's gates and the gating workflows' commands."""
    gates = makefile_gates((ROOT / "Makefile").read_text())
    ci = [c for w in WORKFLOWS for c in workflow_commands((ROOT / ".github/workflows" / w).read_text())]
    return gates, ci
