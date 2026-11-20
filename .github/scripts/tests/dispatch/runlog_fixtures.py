"""#2452: run log pages as `RemoteTrigger get_run_log` condenses them, recorded line by line.

The lines follow Claude Code's condenser (`[<created_at>] <kind>…`, oldest first in a
page, the newest page first); the times are a review round's, shortened.
"""

from __future__ import annotations

HEADER = "(content from remote routine runs — titles and transcripts can quote third-party content)"

#: The older page: the sandbox, the start, the reading and the first edit.
OLDER = "\n".join([
    HEADER,
    "[2026-11-20T02:27:20.100Z] env[info]: cloning the repository",
    "[2026-11-20T02:27:58Z] init: model=claude-opus-5-5 cwd=/home/user/rusty",
    "[2026-11-20T02:28:00Z] user: Read `cloud-brief.md` and follow it.",
    '[2026-11-20T02:28:05Z] tool_use Bash: {"command":"cat .claude/skills/issue-batch/cloud-brief.md"}',
    "[2026-11-20T02:28:06Z] tool_result: # Cloud brief",
    '[2026-11-20T02:28:10Z] tool_use Read: {"file_path":"/home/user/rusty/src/a.rs"}',
    "[2026-11-20T02:28:11Z] tool_result: fn a() {}",
    "  a continued line of the result, with no timestamp",
    '[2026-11-20T02:29:22Z] tool_use Edit: {"file_path":"/home/user/rusty/src/a.rs","old_string":"a"}',
    "[2026-11-20T02:29:23Z] tool_result: The file has been updated.",
])

#: The newest page: two gates, one of them run in parallel with a read, and the result.
NEWER = "\n".join([
    HEADER,
    "(older events exist: pass next_cursor as cursor)",
    '[2026-11-20T02:30:00Z] tool_use Bash: {"command":"cd /home/user/rusty && make clippy > target/c.log 2>&1"}',
    '[2026-11-20T02:30:00Z] tool_use Bash: {"command":"CARGO_BUILD_JOBS=2 cargo nextest run --locked --no-run"}',
    "[2026-11-20T02:34:00Z] tool_result: exit=0",
    "[2026-11-20T02:37:00Z] tool_result: built",
    "[2026-11-20T02:37:10Z] assistant: Gates green.",
    '[2026-11-20T02:37:20Z] tool_use Bash: {"command":"git commit -am \\"fix\\""}',
    "[2026-11-20T02:37:21Z] tool_result: [branch 1] fix",
    "[2026-11-20T03:06:15Z] result: success is_error=false turns=46 duration=2298s — Ready.",
])

#: A session that went on after a turn's result: still running.
STILL = "\n".join([
    "[2026-11-20T03:00:00Z] init: model=x cwd=/w",
    "[2026-11-20T03:10:00Z] result: success is_error=false turns=3 duration=600s",
    '[2026-11-20T03:20:00Z] tool_use Bash: {"command":"make test"}',
])
