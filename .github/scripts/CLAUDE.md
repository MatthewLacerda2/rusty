# CLAUDE.md — the merge and batch tooling

This file loads when you work under `.github/scripts/`: the Python behind `make queue`,
`make mergeable`, `make ruleset`, `make blockers`, `make reclaim`, `make checks`, `make board`,
`make coder`, `make review-level` and `make mutants-remote`, and the CI helpers
beside them. The root `CLAUDE.md` keeps the
project-wide rules (merging is serialized, gates vs. signals, issues and labels); the
`ci-merge` and `issue-batch` skills say how to *run* this tooling. This file is what a
coder *changing* it must know.

## Shape
- **Python, stdlib only.** Each script is a few `gh` and `git` calls and a verdict: no
  third-party import, nothing to install, so it runs as checked out on any machine and
  in any cloud session.
- **`.github/scripts/merging/` is the merge path, one responsibility per module** (#926).
  `merging/verdict/` answers "may this pull request merge?" (`make mergeable`, and the
  queue before every merge); `merging/queue/` merges `main` in, checks, pushes, waits
  and lands; `merging/watch/` is `--watch`'s loop (order, aging, hand-backs, the lock, how
  it ends). `mergeable.py` and `merge-queue.py` are only the command lines. A new
  concern is a new module, not a longer one.
- **`.github/scripts/dispatch/` is the batch's dispatch steps** (#1569): the brief
  and its model, the coder's routine, the routines API, the review level.
  `coder.py` and `review-level.py` are only the command lines. The routines API
  is reached through `claude mcp serve` (`dispatch/remote.py` says why), never
  with a token a script reads: the claude.ai login stays inside `claude`.
- **GitHub over REST, so the scripts run in cloud sessions.** There GraphQL answers
  403, and `gh pr view/list`, `gh issue list` and `gh repo view` with `--json` are
  GraphQL. New calls go through `gh api repos/{owner}/{repo}/…`, paging with an
  explicit `page=` (`--paginate` failed through the proxy too); `ghrest.py` does both.
  `.github/scripts/tests/test_rest_only.py` fails on a `gh pr|issue|repo …` call (#970); an exception
  goes on its line as `# graphql-ok: <reason>`.
- **The incidents live in the docstrings.** A module's docstring names the incident
  behind each rule it keeps (an issue, a pull request, a dated scorsese run). Keep it
  when you change the rule; write one when you add a rule. Prose is free under the
  size gate, code is not (below).

## Gates on this code
- **`make scripts` is a hard gate** (in `make gates` and `lint.yml`): `unittest` over
  `.github/scripts/tests/`. Tests are grouped there by area — `verdict/`, `queue/`,
  `watch/`, `reclaim/`, `board/`, `checks/`, `mutants/`, `dispatch/` — each a package; a new area
  gets its own folder. A test never reaches GitHub or pushes: it fakes `gh` and `git` the way
  `.github/scripts/tests/watch/fakes.py` and the `fixtures.py` files do.
- **The size gate measures `.py`** (#926): 300 lines of code, 150 for a file under a
  `tests/` directory. Blank lines, `#` comments and docstrings are free; a string that
  is data (`X = """…"""`) is code. A violation says `(lines of code)`, so don't compare
  it with `wc -l`. Nothing is grandfathered: an oversized file is split.
- **`make scripts` also holds CI to what the tooling assumes**: every `make gates`
  command appears in `ci.yml`/`lint.yml` (`test_gate_mirror.py`, #964), and every
  suite job sets `RUSTY_REQUIRE_GPU=1` with the GPU caps kept equal
  (`test_gpu_config.py`, #978). A workflow edit that breaks either fails here.

## The watch
- **It ends itself when its own code changes on `main`** (#967,
  `merging/watch/freshness.py`). At launch it notes the blob of every module of the
  `merging` package it loaded, plus the entry script; before each take it compares
  them with `origin/main` and exits `relaunch: …` with status 5. Code the watch runs
  any other way is not tracked, so keep its code inside `merging/`.
- **One watch at a time** (#928, `merging/watch/lock.py`): an `fcntl.flock` on
  `.git/merge-queue.lock` in the common git directory, held for the life of the
  process; a second watch exits with status 4, naming the holder's PID. The kernel
  drops the lock however the process dies, which is why it is a lock and not a PID
  file.
- **Stop a watch by its PID, never by pattern.** scorsese runs the same
  `merge-queue.py --watch` command line on this machine, and on 2026-10-05 a `pkill -f`
  meant for rusty's watch killed scorsese's too. Find rusty's by its working directory
  (`lsof -a -p <pid> -d cwd`) and kill that PID.
- **Its exit statuses are an interface**: 1 (read the hand-backs), 3 (the machine or
  GitHub failed), 4 (another watch holds the lock), 5 (relaunch), and a last line
  `watch: resume: <command>`. The skills and the orchestrator read them, so a change
  to one updates `ci-merge` in the same pull request.
- **The supervisor** (`queue-daemon.py`, #908) adds two statuses of its own, 7 (its
  worktree is gone) and 8 (it crashed), each one `supervisor: … stopped.` line, never
  a traceback (#1450). It holds `git worktree lock` on its worktree for its life
  (`merging/watch/tenancy.py`), which `reclaim.py` reads back: keep the reason's
  format (`merge-queue supervisor pid <pid>`) and `tenancy.pid_of` in step.
