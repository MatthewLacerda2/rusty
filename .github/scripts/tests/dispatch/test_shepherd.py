"""#2629: `make shepherd PR=N` starts one cloud routine per turn of a pull request's review loop.

Must start one, named apart from the coders and with the `Agent` tool, for a ready pull request
with no record; must refuse a stop, a pull request whose shepherd is still in flight, and answer 3
at the cap; a shepherd that has posted its record, or is older than `BUSY`, no longer counts.
`--stage` says where the loop stands and the head to review; `--record --dry-run` on a merged pull
request prints a record `merging/verdict` accepts, and without `--dry-run` refuses one at a stop;
`--record` refuses, printing nothing, when the head moved past the one reviewed by commits of its own,
and stamps the reviewed head when only a merge of `main` followed it.
"""

from __future__ import annotations

import contextlib
import io
import tempfile
import unittest
from datetime import datetime, timedelta, timezone
from unittest import mock

from dispatch import running, shepherd
from merging.verdict import reviewed, standing
from tests import load

cli = load("shepherd.py", "shepherd_cli")
NOW = datetime(2026, 11, 23, 12, 0, tzinfo=timezone.utc)
HEAD = "d" * 40
ENV = {"data": [{"name": "rusty #8 earlier", "created_at": "2026-11-20T00:00:00Z",
                 "job_config": {"ccr": {"environment_id": "env_cloud"}}}]}


def made(number: int, minutes: int) -> dict:
    return {"name": f"{shepherd.NAME_PREFIX}{number} x", "created_at": (NOW - timedelta(minutes=minutes)).isoformat()}


class Fake:
    def __init__(self, state="open", comments=(), routines=(), threads=None, head=HEAD, compare=None):
        self.state, self.comments, self.threads, self.head, self.compare = state, list(comments), threads or {}, head, compare
        self.routines, self.calls = {"data": ENV["data"] + list(routines)}, []

    def gh(self, path: str):
        if "page=" in path and "page=1" not in path:
            return []
        if path.endswith("/pulls/7"):
            return {"number": 7, "title": "Do it", "state": self.state, "draft": False, "head": {"sha": self.head},
                    "merged_at": "t" if self.state == "merged" else None, "labels": []}
        if "/pulls/7/files" in path:
            return [{"filename": "src/a.rs"}]
        if "/compare/" in path:
            return self.compare
        if "/comments" in path:
            return self.threads.get(int(path.split("/issues/")[1].split("/")[0]), self.comments)
        return None

    def call(self, arguments: dict):
        self.calls.append(arguments)
        return (200, self.routines) if arguments["action"] == "list" else (201, {"id": "trig_1"})

    def run(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err), \
                mock.patch("ghrest.slug", return_value="o/r"):
            code = cli.main(list(argv), gh=self.gh, call=self.call, now=NOW, env={})
        return code, out.getvalue(), err.getvalue()

    def created(self) -> list[dict]:
        return [c["body"] for c in self.calls if c["action"] == "create"]


class Start(unittest.TestCase):
    def test_a_ready_pull_request_with_no_record_gets_one(self):
        fake = Fake()
        code, out, _ = fake.run("7")
        self.assertEqual(code, 0, out)
        body = fake.created()[0]
        self.assertTrue(body["name"].startswith(shepherd.NAME_PREFIX + "7 "))
        self.assertIsNone(running.issue(body), "a coder slot would count it")
        ccr = body["job_config"]["ccr"]
        self.assertIn("Agent", ccr["session_context"]["allowed_tools"])
        self.assertIn(".claude/agents/shepherd.md", ccr["events"][0]["data"]["message"]["content"])
        self.assertEqual(ccr["environment_id"], "env_cloud")

    def test_a_stop_starts_none(self):
        fake = Fake(state="merged")
        code, _, err = fake.run("7")
        self.assertEqual((code, fake.created()), (1, []))
        self.assertIn("stage: stop: it is merged", err)

    def test_one_in_flight_on_it_refuses_a_second(self):
        code, _, err = Fake(routines=[made(7, 10)]).run("7")
        self.assertEqual(code, 1)
        self.assertIn("still in flight", err)

    def test_the_cap_answers_three(self):
        fake = Fake(routines=[made(5, 10), made(6, 20)])
        code, _, err = fake.run("7")
        self.assertEqual((code, fake.created()), (3, []))
        self.assertIn("the cap is 2", err)

    def test_a_shepherd_that_posted_or_is_old_frees_its_count(self):
        posted = {5: [{"body": "Independent review\nVerdict: queue on abc1234", "author_association": "OWNER",
                       "created_at": NOW.isoformat()}]}
        fake = Fake(routines=[made(5, 10), made(6, shepherd.BUSY.seconds // 60 + 1)], threads=posted)
        self.assertEqual(fake.run("7")[0], 0)


class Record(unittest.TestCase):
    def test_stage_says_where_the_loop_stands(self):
        self.assertEqual(Fake().run("7", "--stage")[1],
                         f"stage: review: no review record: review ddddddd (rounds sent: 0 of 2); head {HEAD}\n")

    def setUp(self):
        with tempfile.NamedTemporaryFile("w", suffix=".md", delete=False) as text:
            text.write("Independent review (medium: the paths named no trigger)\n\nNo findings.\n")
        self.text = text.name

    def post(self, fake: Fake, reviewed: str = HEAD, *more: str) -> tuple[int, str, str]:
        return fake.run("7", "--record", self.text, "--verdict", "queue", "--reviewed", reviewed, *more)

    def test_a_head_moved_past_the_one_reviewed_gets_no_record(self):
        h1, h2 = "1" * 40, "2" * 40
        own = {"ahead_by": 2, "commits": [{"sha": h1, "parents": [{"sha": "0" * 40}]},
                                          {"sha": h2, "parents": [{"sha": h1}]}]}
        code, out, err = self.post(Fake(head=h2, compare=own), h1)
        self.assertEqual((code, out), (1, ""))
        self.assertIn("review 2222222", err)
        self.assertEqual(self.post(Fake(head=h2, compare=None), h1)[:2], (1, ""), "an unread compare is a doubt")
        merge = {"ahead_by": 2, "commits": [own["commits"][0], {"sha": h2, "parents": [{"sha": h1}, {"sha": "9" * 40}]}]}
        code, out, _ = self.post(Fake(head=h2, compare=merge), h1)
        self.assertEqual(code, 0)
        self.assertTrue(out.rstrip().endswith("Verdict: queue on 1111111"), out)

    def test_the_dry_run_on_a_merged_pull_request_prints_a_record_the_queue_accepts(self):
        code, out, err = self.post(Fake(state="merged"), HEAD, "--dry-run")
        self.assertEqual(code, 0, err)
        self.assertIn("dry run", err)
        comment = {"body": out, "author_association": "OWNER", "created_at": "t"}
        self.assertTrue(reviewed.decided([comment]))
        self.assertEqual(standing.said([comment]), ("queue", HEAD[:7]))
        self.assertEqual(self.post(Fake(state="merged"))[:2], (1, ""))

    def test_usage(self):
        for argv in ((), ("x",), ("7", "--record", "f"), ("7", "--verdict"),
                     ("7", "--record", "f", "--verdict", "queue"), ("7", "--record", "f", "--verdict", "queue", "--reviewed", "zz")):
            self.assertEqual(Fake().run(*argv)[0], 1, argv)


if __name__ == "__main__":
    unittest.main()
