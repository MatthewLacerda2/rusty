"""What `make queue-quiet` does itself, so nobody is woken for it (#2628).

After each merge the orchestrator fast-forwarded its main checkout and ran
`make reclaim`, and once a minute it ran `make coders`: three commands a script
can run. Each returns the lines of what failed, and nothing when it worked.

- **The main checkout moves only forward, only on `main`** ([`forward`]):
  `git merge --ff-only origin/main` there, after a fetch; a checkout on another
  branch, or one whose changes the merge would overwrite, is said and left as it
  is, never reset.
- **`make reclaim` keeps its own guards** (unpushed work, a lock, a worktree in
  use): this only runs it, from the main checkout, and says its last line when
  it fails.
- **The drain pass is `coders.py` itself**, under its own lock
  (`dispatch/drain.py`), so a pass the orchestrator also runs launches nothing
  twice.
"""

from __future__ import annotations

import subprocess
from pathlib import Path

from merging.queue import gitio

# How long one chore may run before it is said to have hung.
TIMEOUT_SECONDS = 600

PREFIX = "QUIET:"


def main_checkout(common_dir: Path) -> Path:
    """The main working tree: the parent of the checkout's common `.git`."""
    return common_dir.resolve().parent


def last(text: str) -> str:
    lines = [line.strip() for line in (text or "").splitlines() if line.strip()]
    return lines[-1] if lines else "no output"


def run(command: list[str], cwd: Path) -> tuple[int, str]:
    """`command`'s status and its last line of output; 124 when it hung."""
    try:
        done = subprocess.run(command, cwd=cwd, capture_output=True, text=True, check=False, timeout=TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        return 124, f"still running after {TIMEOUT_SECONDS} s"
    except OSError as failed:
        return 127, str(failed)
    return done.returncode, (last(done.stderr or done.stdout) if done.returncode else "")


def forward(main: Path, number: int, git=gitio.git) -> list[str]:
    """Fast-forward the main checkout to `origin/main` after #`number` merged."""
    branch = git("symbolic-ref", "--quiet", "--short", "HEAD", cwd=str(main))
    if branch.returncode != 0 or branch.stdout.strip() != "main":
        return [f"{PREFIX} #{number} merged; the main checkout {main} is not on `main`, so it was not moved."]
    fetched = git("fetch", "--quiet", "origin", "main", cwd=str(main))
    if fetched.returncode != 0:
        return [f"{PREFIX} #{number} merged; fetching `main` into {main} failed: {last(fetched.stderr)}"]
    moved = git("merge", "--ff-only", "--quiet", "origin/main", cwd=str(main))
    if moved.returncode != 0:
        return [f"{PREFIX} #{number} merged; {main} could not fast-forward to `origin/main`: {last(moved.stderr)}"]
    return []


def after_merge(main: Path, number: int) -> list[str]:
    """[`forward`], then `make reclaim`."""
    said = forward(main, number)
    status, why = run(["python3", ".github/scripts/reclaim.py"], main)
    if status:
        said.append(f"{PREFIX} #{number} merged; `make reclaim` failed (exit {status}): {why}")
    return said


def drain(main: Path) -> list[str]:
    """One `make coders` pass."""
    status, why = run(["python3", ".github/scripts/coders.py"], main)
    return [f"{PREFIX} the coders' drain pass (`make coders`) failed (exit {status}): {why}"] if status else []
