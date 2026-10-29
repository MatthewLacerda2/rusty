"""Why the queue's push was refused, for the hand-back ([`explain`], #1108).

Three things can refuse the queue's push, and each goes somewhere different:

- **GitHub** (`[remote rejected]`): a token without the `workflow` scope, a
  ruleset (GH013), a 403. Nothing local reproduces it.
- **The pre-push hook** (`.githooks/pre-push`, on the queue's own pushes since
  #1101): its `resolved:` lines, or a traceback when it could not start a
  command (a `FileNotFoundError` for a `cargo` missing from its PATH). Git then
  prints no ref-status line, since the push never reached GitHub.
- **A moved branch**: something else pushed, reset or deleted the branch since
  the take began, so the lease on its starting head refuses the push (#1568).
  Not broken: the take ends [`MOVED`], and the watch takes the branch again on
  its new head; named pull requests are never retaken ([`taking.take`] words
  it [`MOVED_NAMED`] and hands it back).

Before #1108 every refusal read as the lease (the force-push's guard then), and
the orchestrator went looking for a foreign push that never happened. The stderr decides first, so a push
landing between the refusal and the `ls-remote` cannot turn a hook refusal into
a moved branch (#1118's independent review); only a refusal the stderr does
not name falls back to the remote head: still the head the take began from,
so not moved, so the hook; a head that could not be read, [`UNREAD`], a
hand-back (an expired token would otherwise read as moved and the watch retake
it forever); anything else, moved.

The first note is the hand-back's one-line reason ([`taking.take`]); the full
stderr follows it, since a rustfmt diff is no reason line.
"""

from __future__ import annotations

REMOTE = "remote"
HOOK = "hook"
MOVED_BRANCH = "moved"
UNREAD = "unread"

# How a take refused because its branch moved ends: [`taking.take`] reads it as
# "not ready", never a hand-back, and the watch takes the new head (#1568).
MOVED = "its branch moved during the take; it is taken again on its new head."

# The same refusal under `make queue PRS=…`, which never retakes: not merged.
MOVED_NAMED = "its branch moved during the take; not merged. Name it again to take its new head."

REMOTE_REJECTED = "[remote rejected]"
HOOK_LINE = "resolved:"
TRACEBACK = "Traceback (most recent call last):"


def lines(stderr: str) -> list[str]:
    return [line.strip() for line in stderr.splitlines() if line.strip()]


def status_line(line: str) -> bool:
    """Whether `line` is git's per-ref status (` ! [rejected] …`)."""
    return line.startswith("! [")


def cause(stderr: str, held: bool | None) -> str:
    """[`REMOTE`], [`HOOK`], [`UNREAD`] or [`MOVED_BRANCH`]. `held`: GitHub still
    has the take's starting head; `None` when its head could not be read."""
    said = lines(stderr)
    if any(REMOTE_REJECTED in line for line in said):
        return REMOTE
    if any(line.startswith(HOOK_LINE) for line in said):
        return HOOK
    if TRACEBACK in stderr and not any(status_line(line) for line in said):
        return HOOK
    if held is None:
        return UNREAD
    return HOOK if held else MOVED_BRANCH


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


def explain(branch: str, stderr: str, fresh: str, held: bool | None) -> list[str]:
    """The notes for a refused push of `branch` whose updated head was `fresh`;
    the first is [`MOVED`] when the branch moved."""
    why = cause(stderr, held)
    notes = {
        REMOTE: [
            f"GitHub rejected the push of {branch}: {gist(stderr, why)}",
            "A permission or a ruleset, not the code: nothing local reproduces it. Handed back.",
        ],
        HOOK: [
            f"the pre-push hook refused the push of {branch}: {gist(stderr, why)}",
            f"The updated head was {fresh[:7]}, built in a worktree since removed: merge"
            f" `origin/main` into {branch} and run `make resolved` to see it. Handed back.",
        ],
        UNREAD: [
            f"the push of {branch} was refused and its head on GitHub could not be read: {gist(stderr, why)}",
            "Not known to have moved, so not taken again: check the token and the branch. Handed back.",
        ],
        MOVED_BRANCH: [
            MOVED,
            f"The push of {branch} was refused by its lease: {gist(stderr, why)}",
        ],
    }[why]
    if why == HOOK and "FileNotFoundError" in stderr:
        notes.insert(1, "The hook could not start a command; is `cargo` on its PATH?")
    return [*notes, *(["git said:", *stderr.strip().splitlines()] if stderr.strip() else [])]
