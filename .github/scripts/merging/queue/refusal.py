"""Why a force-push was refused, for the hand-back ([`explain`], #1108).

Three things can refuse the queue's push, and each sends the orchestrator
somewhere different:

- **GitHub** (`[remote rejected]`): a token without the `workflow` scope, a
  ruleset (GH013), a 403. Nothing local reproduces it.
- **The pre-push hook** (`.githooks/pre-push`, on the queue's own pushes since
  #1101): its `resolved:` lines, or a traceback when it could not start a
  command (a `FileNotFoundError` for a `cargo` missing from its PATH). Git then
  prints no ref-status line, since the push never reached GitHub.
- **The lease**: something else pushed since the take began.

Before #1108 every refusal read as the lease, and the orchestrator went looking
for a foreign push that never happened. The stderr decides first, so a push
landing between the refusal and the `ls-remote` cannot turn a hook refusal into
the lease (#1118's independent review); only a refusal the stderr does not
name falls back to the remote head: still the head the take began from, the
lease held, so the hook; anything else, the lease.

The first note is the hand-back's one-line reason ([`taking.take`]); the full
stderr follows it, since a rustfmt diff is no reason line.
"""

from __future__ import annotations

REMOTE = "remote"
HOOK = "hook"
LEASE = "lease"

REMOTE_REJECTED = "[remote rejected]"
HOOK_LINE = "resolved:"
TRACEBACK = "Traceback (most recent call last):"


def lines(stderr: str) -> list[str]:
    return [line.strip() for line in stderr.splitlines() if line.strip()]


def status_line(line: str) -> bool:
    """Whether `line` is git's per-ref status (` ! [rejected] …`)."""
    return line.startswith("! [")


def cause(stderr: str, held: bool) -> str:
    """[`REMOTE`], [`HOOK`] or [`LEASE`]. `held`: GitHub still has the take's starting head."""
    said = lines(stderr)
    if any(REMOTE_REJECTED in line for line in said):
        return REMOTE
    if any(line.startswith(HOOK_LINE) for line in said):
        return HOOK
    if TRACEBACK in stderr and not any(status_line(line) for line in said):
        return HOOK
    return HOOK if held else LEASE


def gist(stderr: str, why: str) -> str:
    """The one line that says it: the remote's reason, the hook's lines, or a traceback's last."""
    said = lines(stderr)
    if why == REMOTE:
        picked = [line for line in said if REMOTE_REJECTED in line]
    elif why == HOOK:
        # The hook's own lines, else a traceback's last: git's closing `error:` says nothing.
        picked = [line for line in said if line.startswith(HOOK_LINE)] or [
            line for line in said if not status_line(line) and not line.startswith("error:")
        ][-1:]
    else:
        picked = [line for line in said if status_line(line)]
    return "; ".join(picked or said[:1]) or "git said nothing"


def explain(branch: str, stderr: str, fresh: str, held: bool) -> list[str]:
    """The hand-back notes for a refused push of `branch` whose rebased head was `fresh`."""
    why = cause(stderr, held)
    notes = {
        REMOTE: [
            f"GitHub rejected the force-push of {branch}: {gist(stderr, why)}",
            "A permission or a ruleset, not the code: nothing local reproduces it. Handed back.",
        ],
        HOOK: [
            f"the pre-push hook refused the force-push of {branch}: {gist(stderr, why)}",
            f"The rebased head was {fresh[:7]}, built in a worktree since removed: rebase"
            f" {branch} onto `origin/main` and run `make resolved` to see it. Handed back.",
        ],
        LEASE: [
            f"the force-push of {branch} was refused: {gist(stderr, why)}",
            "The lease held the head this queue started from, so"
            " something else has pushed since. Handed back.",
        ],
    }[why]
    if why == HOOK and "FileNotFoundError" in stderr:
        notes.insert(1, "The hook could not start a command; is `cargo` on its PATH?")
    return [*notes, *(["git said:", *stderr.strip().splitlines()] if stderr.strip() else [])]
