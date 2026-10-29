"""#1569: the coder's routine is built the way `issue-batch` described it by hand."""

from __future__ import annotations

import unittest
from datetime import datetime, timedelta, timezone

from dispatch import routine

NOW = datetime(2026, 10, 29, 14, 3, 41, tzinfo=timezone.utc)


def made(name: str, env: str, at: str) -> dict:
    return {"name": name, "created_at": at, "job_config": {"ccr": {"environment_id": env}}}


class Time(unittest.TestCase):
    def test_fires_ahead_on_the_minute_in_utc(self):
        self.assertEqual(routine.fire_time(NOW), "2026-10-29T14:05:00Z")
        local = NOW.astimezone(timezone(timedelta(hours=-3)))
        self.assertEqual(routine.fire_time(local), "2026-10-29T14:05:00Z")

    def test_never_fires_in_the_past(self):
        at = datetime.strptime(routine.fire_time(NOW), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
        self.assertGreater(at, NOW + timedelta(minutes=1))


class Name(unittest.TestCase):
    def test_short_titles_are_whole(self):
        self.assertEqual(routine.name(12, "Fix  the\nthing"), "rusty #12 Fix the thing")

    def test_long_titles_are_cut_at_a_word(self):
        cut = routine.name(1569, "make coder ISSUE=N and make review-level PR=N: dispatch without hand-built calls")
        self.assertLessEqual(len(cut), routine.NAME_LENGTH)
        self.assertTrue(cut.startswith("rusty #1569 make coder"))
        self.assertFalse(cut.endswith(" "))


class Environment(unittest.TestCase):
    def test_the_newest_coder_routine_names_it(self):
        routines = [
            made("rusty #1 old", "env_old", "2026-10-01T00:00:00Z"),
            made("weekly report", "env_other", "2026-10-29T00:00:00Z"),
            made("rusty #2 new", "env_new", "2026-10-28T00:00:00Z"),
        ]
        self.assertEqual(routine.environment(routines), "env_new")

    def test_none_without_a_coder_routine(self):
        self.assertIsNone(routine.environment([made("weekly report", "env_other", "2026-10-29T00:00:00Z")]))
        self.assertIsNone(routine.environment([]))


class Body(unittest.TestCase):
    def setUp(self):
        self.body = routine.body(42, "A title", "claude-sonnet-5-5", "env_x", "o/r", NOW, event_id="e-1")
        self.ccr = self.body["job_config"]["ccr"]

    def test_the_prompt_is_the_fixed_pointer_at_the_issue(self):
        message = self.ccr["events"][0]["data"]["message"]
        self.assertEqual(message["role"], "user")
        self.assertIn("cloud-brief.md", message["content"])
        self.assertIn("Issue(s): #42.", message["content"])
        self.assertIn("newest comment on it that starts with `Brief`", message["content"])

    def test_model_environment_repository_and_tools(self):
        context = self.ccr["session_context"]
        self.assertEqual(context["model"], "claude-sonnet-5-5")
        self.assertEqual(self.ccr["environment_id"], "env_x")
        self.assertEqual(context["sources"], [{"git_repository": {"url": "https://github.com/o/r"}}])
        self.assertIn("ToolSearch", context["allowed_tools"])

    def test_runs_once_and_is_named_for_the_issue(self):
        self.assertEqual(self.body["run_once_at"], "2026-10-29T14:05:00Z")
        self.assertNotIn("cron_expression", self.body)
        self.assertEqual(self.body["name"], "rusty #42 A title")
        self.assertTrue(self.body["enabled"])

    def test_each_launch_gets_its_own_event_id(self):
        one = routine.body(42, "t", "m", "e", "o/r", NOW)["job_config"]["ccr"]["events"][0]["data"]["uuid"]
        two = routine.body(42, "t", "m", "e", "o/r", NOW)["job_config"]["ccr"]["events"][0]["data"]["uuid"]
        self.assertNotEqual(one, two)


if __name__ == "__main__":
    unittest.main()
