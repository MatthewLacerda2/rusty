"""A workflow's top-level jobs, read one way by every check under `tests/`.

`test_gpu_config.py`, `test_workflow_hygiene.py` and `ruleset/test_reporting.py`
each split a workflow into its jobs; three copies of that split had drifted (one
dropped comment lines, one did not, none read a job heading with a comment on
it). This is the one copy. It reads the shape the repository's workflows are
written in, block YAML with two-space indents, not every spelling YAML allows,
so a reader of a single value (`runs_on`) says it cannot read one rather than
guess: a check built on it fails on what it cannot read, never passes.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

WORKFLOW_DIR = Path(__file__).resolve().parents[2] / "workflows"

TOP = re.compile(r"^jobs: *(?:#.*)?$")
#: A job's heading: a two-space key with nothing after it but a comment.
HEADING = re.compile(r"^  ([\w-]+): *(?:#.*)?$")
COMMENT = re.compile(r"(?:^|\s+)#.*$")


@dataclass
class Job:
    name: str
    #: The heading's line number, 1-based.
    line: int
    #: The body's lines and their numbers; whole-line comments dropped.
    lines: list[tuple[int, str]] = field(default_factory=list)

    @property
    def body(self) -> str:
        return "".join(f"{text}\n" for _, text in self.lines)


def parse(text: str) -> dict[str, Job]:
    """Each top-level job under `jobs:`, in order."""
    found: dict[str, Job] = {}
    job, inside = None, False
    for number, line in enumerate(text.splitlines(), 1):
        if TOP.match(line):
            inside = True
        elif not inside or line.lstrip().startswith("#") or not line.strip():
            continue
        elif not line.startswith("  "):
            inside, job = False, None  # a top-level key after `jobs:`
        elif m := HEADING.match(line):
            job = found[m[1]] = Job(m[1], number)
        elif job:
            job.lines.append((number, line))
    return found


def jobs(text: str) -> dict[str, str]:
    """Each top-level job's name and its body, whole-line comments dropped."""
    return {name: job.body for name, job in parse(text).items()}


def read(workflow: str) -> str:
    return (WORKFLOW_DIR / workflow).read_text()


def repo_workflows() -> dict[str, str]:
    """Every workflow GitHub runs, by file name: `*.yml` and `*.yaml`."""
    return {p.name: p.read_text() for p in sorted(WORKFLOW_DIR.iterdir()) if p.suffix in (".yml", ".yaml")}


class Unreadable(ValueError):
    """A `runs-on` this reader does not understand, with the line it is on."""

    def __init__(self, line: int, why: str):
        super().__init__(f"line {line}: {why}")
        self.line, self.why = line, why


MATRIX = re.compile(r"^\$\{\{ *matrix\.([\w-]+) *\}\}$")


def unquote(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
        return value[1:-1]
    return value


def runs_on(job: Job) -> list[str] | None:
    """The images a job runs on: one, or each value of the matrix key it names
    (`${{ matrix.os }}`, read from the job's `os:` entries). `None` for a job
    with no `runs-on` (a reusable workflow's `uses:`). Raises `Unreadable` for a
    list, a mapping, another expression, or a matrix key with no value written."""
    for number, line in job.lines:
        if not line.startswith("    runs-on:"):
            continue
        value = unquote(COMMENT.sub("", line.split(":", 1)[1]).strip())
        if m := MATRIX.match(value):
            key = re.compile(rf"(?:^\s*-?\s*|[{{,]\s*){re.escape(m[1])}: *([^\s,}}#]+)")
            values = [unquote(v) for _, text in job.lines for v in key.findall(text)]
            if not values:
                raise Unreadable(number, f"`{value}` names a matrix key with no value written in the job")
            return values
        if not re.fullmatch(r"[\w.-]+", value):
            raise Unreadable(number, f"`runs-on` must be one image name, read `{value or '(empty)'}`")
        return [value]
    return None
