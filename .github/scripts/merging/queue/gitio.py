"""`git`, and the calls to GitHub through it retried on a network blip (#875)."""

from __future__ import annotations

import subprocess
import time

from merging.verdict import github


def git(*args: str, cwd: str | None = None) -> subprocess.CompletedProcess:
    """`git`, captured and never fatal. Callers read `returncode` themselves."""
    return subprocess.run(["git", *args], capture_output=True, text=True, check=False, cwd=cwd)


# What `git` prints when the network failed, not the request (#875: one SSL
# timeout on a force-push handed back a healthy branch). Beside `gh`'s wording
# ([`verdict.github.TRANSIENT`]), which git shares in part.
GIT_TRANSIENT = (
    "ssl connection timeout",
    "could not resolve host",
    "connection timed out",
    "operation timed out",
    "failed to connect to",
    "early eof",
    "the remote end hung up unexpectedly",
    "rpc failed",
    "gnutls_handshake() failed",
)


def git_transient(stderr: str) -> bool:
    """Whether `git` failed for the network's reasons rather than the request's."""
    low = stderr.lower()
    return github.transient(stderr) or any(sign in low for sign in GIT_TRANSIENT)


def remote(*args: str, cwd: str | None = None) -> subprocess.CompletedProcess:
    """[`git`] for a call that talks to GitHub (`fetch`, `push`, `ls-remote`).

    A transient failure is asked again, exactly as given, on `mergeable.gh`'s
    backoff ([`verdict.github.RETRIES`]); still failing after the last raises
    [`verdict.github.Unreachable`], which [`stopping.drain`] turns into a machine stop. Any
    other result is returned for the caller to read, as [`git`]'s is.
    """
    for wait in (*github.RETRIES, None):
        done = git(*args, cwd=cwd)
        if done.returncode == 0 or not git_transient(done.stderr):
            return done
        if wait is None:
            raise github.Unreachable(f"git {' '.join(args)}: {done.stderr.strip()}")
        time.sleep(wait)
    raise AssertionError("unreachable: the loop returns or raises")


def remote_head(branch: str, cwd: str) -> str | None:
    """`branch`'s head on GitHub, `None` when git could not say."""
    listed = remote("ls-remote", "origin", f"refs/heads/{branch}", cwd=cwd)
    words = listed.stdout.split() if listed.returncode == 0 else []
    return words[0] if words else None
