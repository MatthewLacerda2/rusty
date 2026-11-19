#!/usr/bin/env python3
"""The least memory a runner had free while the suite ran (#2363).

The `gpu` test group's cap (`.config/nextest.toml`) is how many headless renderers
exist at once, and a software adapter (lavapipe) allocates each in system RAM, so
raising the cap is only safe with the runner's memory in view. A suite job starts
this in the background before its test step and the "Slowest tests" step prints
the result beside the shard's accounting:

    python3 .github/scripts/memory_floor.py watch mem-floor.txt &   # before the suite
    python3 .github/scripts/memory_floor.py report mem-floor.txt    # after it

`watch` samples every second and rewrites the file with the lowest available
memory seen, the total and the sample count; `report` prints one `memory:` line.
Available memory is Linux's `MemAvailable` (`/proc/meminfo`) and, on macOS,
`vm_stat`'s free, inactive, speculative and purgeable pages: what the kernel can
hand out without swapping. A sampler that could not read the memory, or never
ran, prints why and exits 0: this informs, it never gates.
"""

from __future__ import annotations

import re
import subprocess
import sys
import time
from pathlib import Path

#: `vm_stat`'s pages that can be handed out without swapping.
MAC_AVAILABLE = ("Pages free", "Pages inactive", "Pages speculative", "Pages purgeable")


def linux(meminfo: str) -> tuple[int, int]:
    """(available, total) KiB from `/proc/meminfo`'s text."""
    fields = dict(re.findall(r"^(\w+):\s+(\d+) kB$", meminfo, flags=re.M))
    return int(fields["MemAvailable"]), int(fields["MemTotal"])


def mac(vm_stat: str, memsize: str) -> tuple[int, int]:
    """(available, total) KiB from `vm_stat`'s and `sysctl -n hw.memsize`'s text."""
    page = int(re.search(r"page size of (\d+) bytes", vm_stat)[1])
    pages = dict(re.findall(r'^"?([\w ]+?)"?:\s+(\d+)\.$', vm_stat, flags=re.M))
    return sum(int(pages.get(k, 0)) for k in MAC_AVAILABLE) * page // 1024, int(memsize) // 1024


def sample() -> tuple[int, int]:
    """This machine's (available, total) KiB."""
    if sys.platform == "darwin":
        run = lambda *cmd: subprocess.run(cmd, capture_output=True, text=True, check=True).stdout  # noqa: E731
        return mac(run("vm_stat"), run("sysctl", "-n", "hw.memsize"))
    return linux(Path("/proc/meminfo").read_text())


def lower(floor: tuple[int, int, int] | None, now: tuple[int, int]) -> tuple[int, int, int]:
    """The floor (lowest available, total, samples) after one more sample."""
    if floor is None:
        return now[0], now[1], 1
    return min(floor[0], now[0]), now[1], floor[2] + 1


def line(text: str) -> str:
    """The `memory:` line for a floor file's text."""
    try:
        least, total, samples = (int(v) for v in text.split())
    except ValueError:
        return f"memory: no floor ({text.strip()!r} is not `<least KiB> <total KiB> <samples>`)"
    gib = 1024 * 1024
    return (f"memory: at least {least / gib:.1f} GiB of {total / gib:.1f} GiB available "
            f"({100 * least / total:.0f}%) over {samples} one-second samples during the suite")


def watch(path: Path, interval: float = 1.0) -> None:
    floor = None
    while True:
        try:
            floor = lower(floor, sample())
        except (OSError, KeyError, TypeError, ValueError, subprocess.SubprocessError) as err:
            path.write_text(f"unreadable: {err}\n")
            return
        path.write_text(" ".join(map(str, floor)) + "\n")
        time.sleep(interval)


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] not in ("watch", "report"):
        print(f"usage: {argv[0]} watch|report <file>", file=sys.stderr)
        return 2
    path = Path(argv[2])
    if argv[1] == "watch":
        watch(path)
        return 0
    try:
        print(line(path.read_text()))
    except OSError as err:
        print(f"memory: no floor ({err})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
