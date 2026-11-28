"""#2667: `make orchestrator`: the hourly routine, the fire's hold on the lock, and the outbox."""

from __future__ import annotations

import contextlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from dispatch import handed
from orchestrating import lease, schedule
from tests import load
from tests.orchestrating.fakes import CLOUD, Remote

cli = load("orchestrator.py")
LISTED = [{"id": "trig_c", "name": "rusty #8 x", "created_at": "2026-11-27T00:00:00Z",
           "job_config": {"ccr": {"environment_id": "env_cloud"}}}]


class Routines:
    def __init__(self, listed=None):
        self.listed, self.calls = list(LISTED if listed is None else listed), []

    def __call__(self, arguments):
        self.calls.append(arguments)
        if arguments["action"] == "list":
            return 200, {"data": self.listed}
        return 201, {"trigger": {"id": "trig_orch"}}


def run(*argv, call=None, env=None, git=None, root="."):
    out, err = io.StringIO(), io.StringIO()
    with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), contextlib.redirect_stdout(out), \
            contextlib.redirect_stderr(err):
        code = cli.main(list(argv), call=call or Routines(), env=env or {}, git=git, root=root)
    return code, out.getvalue(), err.getvalue()


class Schedule(unittest.TestCase):
    def test_it_creates_one_hourly_routine_pointing_at_the_agent_definition(self):
        call = Routines()
        code, out, _ = run(call=call)
        self.assertEqual(code, 0)
        made = call.calls[-1]["body"]
        self.assertEqual((made["name"], made["cron_expression"]), (schedule.NAME, schedule.CRON))
        self.assertNotIn("run_once_at", made)
        context = made["job_config"]["ccr"]["session_context"]
        self.assertIn(".claude/agents/orchestrator.md", made["job_config"]["ccr"]["events"][0]["data"]["message"]["content"])
        self.assertIn("mcp__Claude_Code_Remote__create_session", context["allowed_tools"])
        self.assertEqual(made["job_config"]["ccr"]["environment_id"], "env_cloud")
        self.assertIn("trig_orch", out)

    def test_a_dry_run_creates_nothing(self):
        call = Routines()
        code, out, _ = run("--dry-run", call=call)
        self.assertEqual((code, [c["action"] for c in call.calls]), (0, ["list"]))
        self.assertEqual(json.loads(out)["cron_expression"], schedule.CRON)

    def test_a_second_schedule_is_refused_and_stop_disables_the_first(self):
        call = Routines(LISTED + [{"id": "trig_orch", "name": schedule.NAME, "enabled": True}])
        self.assertEqual(run(call=call)[0], 1)
        code, out, _ = run("--stop", call=call)
        self.assertEqual(code, 0)
        self.assertEqual(call.calls[-1], {"action": "update", "trigger_id": "trig_orch", "body": {"enabled": False}})

    def test_an_unknown_flag_or_help_creates_nothing(self):
        for argv, status in ((["--help"], 0), (["-n"], 1), (["--dryrun"], 1), (["--stop", "x"], 1)):
            call = Routines()
            self.assertEqual(run(*argv, call=call)[0], status, argv)
            self.assertEqual(call.calls, [], argv)


class Lock(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.root = self.remote.clone("fire")
        self.cloud = {"CLAUDE_CODE_REMOTE_SESSION_ID": "cse_one"}

    def test_a_fire_takes_and_releases_it_as_its_cloud_session(self):
        self.assertEqual(run("take", env=self.cloud, root=self.root)[0], 0)
        code, out, _ = run("status", env=self.cloud, root=self.root)
        self.assertEqual(code, 0)
        self.assertIn(f"held by {CLOUD}", out)
        self.assertEqual(run("release", env=self.cloud, root=self.root)[0], 0)

    def test_a_take_prints_when_the_lease_it_replaced_was_released(self):
        self.assertNotIn("last ", run("take", env=self.cloud, root=self.root)[1])  # never taken before
        run("release", env=self.cloud, root=self.root)
        freed = lease.read(self.remote.side("a", CLOUD).git).at
        code, out, _ = run("take", env={"CLAUDE_CODE_REMOTE_SESSION_ID": "cse_two"}, root=self.root)
        self.assertEqual(code, 0)
        self.assertIn(f"orchestrator: last {lease.stamp(freed)} (free since", out)

    def test_a_fire_finding_another_holder_exits_4_naming_it(self):
        self.remote.side("desk", "local@desk queue-daemon").take()
        code, _, err = run("take", env=self.cloud, root=self.root)
        self.assertEqual(code, lease.HELD_STATUS)
        self.assertIn("held by local@desk queue-daemon", err)
        self.assertEqual(run("status", env=self.cloud, root=self.root)[0], lease.HELD_STATUS)

    def test_the_queue_runs_only_under_the_lock(self):
        code, _, err = run("queue", "20", env=self.cloud, root=self.root)
        self.assertEqual(code, 1)
        self.assertIn("take the lock first", err)


class Outbox(unittest.TestCase):
    def test_it_prints_each_unsent_create_and_marks_one_sent(self):
        made = tempfile.TemporaryDirectory()
        self.addCleanup(made.cleanup)
        file = Path(made.name, "routines.json")
        file.write_text(json.dumps({"environment_id": "env_cloud"}))
        body = schedule.body("env_cloud", "o/r", lease.when("2026-11-28T06:00:00Z"))
        handed.answer({"action": "create", "body": body}, file)
        with mock.patch.dict(os.environ, {handed.ENV: str(file)}):
            code, out, _ = run("outbox")
            self.assertEqual(code, 0)
            printed = json.loads(out)
            self.assertEqual(printed["create_session"]["title"], schedule.NAME)
            self.assertEqual(run("outbox", "--sent", printed["id"])[0], 0)
            self.assertEqual(run("outbox")[1], "")
            self.assertEqual(run("outbox", "--sent", "nope")[0], 1)


if __name__ == "__main__":
    unittest.main()
