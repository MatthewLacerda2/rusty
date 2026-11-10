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
BRIEF = {"body": "Brief\n\nBranch: `9-x`.", "created_at": "2026-10-29T10:00:00Z",
         "author_association": "OWNER", "html_url": "https://github.com/o/r/issues/9#issuecomment-501"}
OLDER = {**BRIEF, "created_at": "2026-10-28T10:00:00Z", "html_url": "https://github.com/o/r/issues/9#issuecomment-400"}
CHAT = {"body": "Thanks!", "created_at": "2026-10-29T11:00:00Z", "author_association": "NONE",
        "html_url": "https://github.com/o/r/issues/9#issuecomment-600"}
ROUTINES = {"data": [{"name": "rusty #8 earlier", "created_at": "2026-10-28T00:00:00Z",
                      "job_config": {"ccr": {"environment_id": "env_cloud"}}}]}


class Fake:
    """GitHub and the routines API, answering from tables and noting the calls."""

    def __init__(self, issue: dict | None = None, comments: list | None = None, create=(200, None),
                 routines: dict | None = None, pulls: list | None = None, threads: dict | None = None):
        self.threads = threads or {}  # a pull request's conversation, by number (#2017)
        self.pulls = [] if pulls is None else pulls
        self.routines = ROUTINES if routines is None else routines
        self.issue = {"number": 9, "title": "Do it", "state": "open", "labels": []} if issue is None else issue
        self.comments = [BRIEF] if comments is None else comments
        self.create = create
        self.calls: list[dict] = []

    def gh(self, path: str):
        if "/pulls" in path:
            return self.pulls if "page=1" in path else []
        if "/comments" in path:
            number = int(path.split("/issues/")[1].split("/")[0])
            return self.threads.get(number, self.comments) if "page=1" in path else []
        return self.issue

    def call(self, arguments: dict):
        self.calls.append(arguments)
        if arguments["action"] == "list":
            return 200, self.routines
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
    def test_creates_the_routine_on_opus_and_prints_its_link(self):
        fake = Fake()
        code, out, _ = fake.run("9")
        self.assertEqual(code, 0)
        [body] = fake.created()
        self.assertEqual(body["job_config"]["ccr"]["session_context"]["model"], "claude-opus-5-5")
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
        self.assertEqual([c["action"] for c in fake.calls], ["list", "create"])
        self.assertEqual(fake.created()[0]["job_config"]["ccr"]["environment_id"], "env_set")

    def test_a_dry_run_prints_the_body_and_creates_nothing(self):
        fake = Fake()
        code, out, _ = fake.run("#9", "--dry-run")
        self.assertEqual(code, 0)
        self.assertEqual(fake.created(), [])
        self.assertEqual(json.loads(out)["name"], "rusty #9 Do it")


def busy(*issues: int) -> dict:
    """A routine list with a fired coder on each of `issues`; its run reads finished (#1866)."""
    run = {"status": "ROUTINE_RUN_STATUS_SUCCEEDED", "fired_at": "2026-10-29T13:00:00Z",
           "finished_at": "2026-10-29T13:30:00Z"}
    return {"data": ROUTINES["data"] + [{"name": f"rusty #{n} x", "enabled": False, "ended_reason": "run_once_fired",
                                         "last_run": run} for n in issues]}


def drafts(*issues: int) -> list[dict]:
    return [{"draft": True, "head": {"ref": f"{n}-x"}} for n in issues]


class Limit(unittest.TestCase):
    def test_a_sixth_coder_is_refused_and_names_the_five(self):
        fake = Fake(routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5))
        code, _, err = fake.run("9")
        self.assertEqual(code, 1)
        self.assertIn("limit is 5", err)
        self.assertIn("rusty #5 x", err)
        self.assertEqual(fake.created(), [])

    def test_four_running_leaves_room_for_a_fifth(self):
        self.assertEqual(Fake(routines=busy(1, 2, 3, 4), pulls=drafts(1, 2, 3, 4)).run("9")[0], 0)

    def test_a_reviewed_ready_pull_request_frees_its_coder(self):
        pulls = drafts(1, 2, 3, 4) + [{"number": 105, "draft": False, "head": {"ref": "5-x"}}]
        record = {105: [{**BRIEF, "body": "No review: data only"}]}
        self.assertEqual(Fake(routines=busy(1, 2, 3, 4, 5), pulls=pulls, threads=record).run("9")[0], 0)

    def test_a_ready_pull_request_in_review_holds_its_slot_and_is_named(self):
        pulls = drafts(1, 2, 3, 4) + [{"number": 105, "draft": False, "head": {"ref": "5-x"}}]
        code, _, err = Fake(routines=busy(1, 2, 3, 4, 5), pulls=pulls, threads={105: [CHAT]}).run("9")
        self.assertEqual(code, 1)
        self.assertIn("rusty #5 x (fired 2026-10-29T13:00Z, in review)", err)

    def test_a_fire_with_no_pull_request_past_two_hours_is_said_and_not_counted(self):
        listed = busy(1, 2, 3, 4)
        listed["data"].append({**listed["data"][-1], "name": "rusty #5 x",
                               "last_run": {"fired_at": "2026-10-29T11:00:00Z"}})
        code, _, err = Fake(routines=listed, pulls=drafts(1, 2, 3, 4)).run("9")
        self.assertEqual(code, 0)
        self.assertIn("rusty #5 x: no PR in 2 h: ended or stalled; not counted", err)

    def test_a_rebrief_of_a_counted_issue_launches(self):
        fake = Fake(routines=busy(1, 2, 3, 4, 9), pulls=drafts(1, 2, 3, 4, 9))
        self.assertEqual(fake.run("9")[0], 0)
        self.assertEqual(len(fake.created()), 1)

    def test_over_limit_launches_anyway_and_says_so(self):
        fake = Fake(routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5))
        code, _, err = fake.run("9", "--over-limit")
        self.assertEqual(code, 0)
        self.assertIn("--over-limit", err)


class Refuse(unittest.TestCase):
    def test_a_refusal_exits_1_and_starts_nothing(self):
        cases = {
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

    def test_unlisted_pull_requests_exit_2(self):
        fake = Fake()
        fake.gh = lambda path: None if "/pulls" in path else Fake().gh(path)
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.created()), (2, []))
        self.assertIn("open pull requests", err)

    def test_a_bad_argument_is_usage(self):
        self.assertEqual(Fake().run("nine")[0], 1)


if __name__ == "__main__":
    unittest.main()
