"""#1569: `make coder` launches on the newest brief, and starts nothing when it refuses."""

from __future__ import annotations

import contextlib
import io
import json
import unittest
from datetime import datetime, timezone
from unittest import mock

from tests import load

coder = load("coder.py")
NOW = datetime(2026, 10, 29, 14, 3, tzinfo=timezone.utc)
BRIEF = {"body": "Brief\n\nBranch: `9-x`.\nModel: Sonnet.", "created_at": "2026-10-29T10:00:00Z",
         "author_association": "OWNER", "html_url": "https://github.com/o/r/issues/9#issuecomment-501"}
OLDER = {**BRIEF, "created_at": "2026-10-28T10:00:00Z", "html_url": "https://github.com/o/r/issues/9#issuecomment-400"}
CHAT = {"body": "Thanks!", "created_at": "2026-10-29T11:00:00Z", "author_association": "NONE",
        "html_url": "https://github.com/o/r/issues/9#issuecomment-600"}
ROUTINES = {"data": [{"name": "rusty #8 earlier", "created_at": "2026-10-28T00:00:00Z",
                      "job_config": {"ccr": {"environment_id": "env_cloud"}}}]}


class Fake:
    """GitHub and the routines API, answering from tables and noting the calls."""

    def __init__(self, issue: dict | None = None, comments: list | None = None, create=(200, None)):
        self.issue = {"number": 9, "title": "Do it", "state": "open", "labels": []} if issue is None else issue
        self.comments = [BRIEF] if comments is None else comments
        self.create = create
        self.calls: list[dict] = []

    def gh(self, path: str):
        if "/comments" in path:
            return self.comments if "page=1" in path else []
        return self.issue

    def call(self, arguments: dict):
        self.calls.append(arguments)
        if arguments["action"] == "list":
            return 200, ROUTINES
        status, answer = self.create
        return status, answer if answer is not None else {"trigger": {"id": "trig_new"}}

    def run(self, *argv: str, env: dict | None = None) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with mock.patch.object(coder.ghrest, "slug", return_value="o/r"), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = coder.main(list(argv), gh=self.gh, call=self.call, now=NOW, env=env or {})
        return code, out.getvalue(), err.getvalue()

    def created(self) -> list[dict]:
        return [c["body"] for c in self.calls if c["action"] == "create"]


class Launch(unittest.TestCase):
    def test_creates_the_routine_on_the_briefs_model_and_prints_its_link(self):
        fake = Fake()
        code, out, _ = fake.run("9")
        self.assertEqual(code, 0)
        [body] = fake.created()
        self.assertEqual(body["job_config"]["ccr"]["session_context"]["model"], "claude-sonnet-5-5")
        self.assertEqual(body["job_config"]["ccr"]["environment_id"], "env_cloud")
        self.assertIn("https://claude.ai/code/routines/trig_new", out)

    def test_the_prompt_names_the_checked_brief_and_no_other_comment(self):
        fake = Fake(comments=[CHAT, BRIEF, OLDER])
        self.assertEqual(fake.run("9")[0], 0)
        prompt = fake.created()[0]["job_config"]["ccr"]["events"][0]["data"]["message"]["content"]
        self.assertIn(BRIEF["html_url"], prompt)
        self.assertNotIn(OLDER["html_url"], prompt)
        self.assertNotIn(CHAT["html_url"], prompt)

    def test_rusty_cloud_env_skips_the_lookup(self):
        fake = Fake()
        self.assertEqual(fake.run("9", env={"RUSTY_CLOUD_ENV": "env_set"})[0], 0)
        self.assertEqual([c["action"] for c in fake.calls], ["create"])
        self.assertEqual(fake.created()[0]["job_config"]["ccr"]["environment_id"], "env_set")

    def test_a_dry_run_prints_the_body_and_creates_nothing(self):
        fake = Fake()
        code, out, _ = fake.run("#9", "--dry-run")
        self.assertEqual(code, 0)
        self.assertEqual(fake.created(), [])
        self.assertEqual(json.loads(out)["name"], "rusty #9 Do it")


class Refuse(unittest.TestCase):
    def test_a_refusal_exits_1_and_starts_nothing(self):
        cases = {
            "no `Model:` line": Fake(comments=[{**BRIEF, "body": "Brief\nBranch: x"}]),
            "planning": Fake(issue={"number": 9, "title": "t", "state": "open", "labels": [{"name": "planning"}]}),
            "no comment starting with": Fake(comments=[]),
        }
        for why, fake in cases.items():
            with self.subTest(why):
                code, _, err = fake.run("9")
                self.assertEqual(code, 1)
                self.assertIn(why, err)
                self.assertEqual(fake.created(), [])

    def test_a_failed_create_exits_2(self):
        code, _, err = Fake(create=(403, {"error": "nope"})).run("9")
        self.assertEqual(code, 2)
        self.assertIn("HTTP 403", err)

    def test_an_unreadable_issue_exits_2(self):
        fake = Fake()
        fake.gh = lambda path: None
        self.assertEqual(fake.run("9")[0], 2)

    def test_a_bad_argument_is_usage(self):
        self.assertEqual(Fake().run("nine")[0], 1)


if __name__ == "__main__":
    unittest.main()
