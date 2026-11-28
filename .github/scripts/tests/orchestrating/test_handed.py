"""#2667: in a cloud session the scripts list from a file and queue their creates for `create_session`."""

from __future__ import annotations

import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from dispatch import handed, remote, routine, running
from tests.dispatch.coder_fakes import NOW, Fake

ENV_ID = "env_cloud"
SESSION = {"id": "session_1", "title": "rusty #8 earlier", "created_at": "2026-10-29T12:00:00Z"}
FIRED = {"id": "session_2", "title": "⚡ rusty #8 earlier", "created_at": "2026-10-29T12:00:00Z"}
TRIGGER = {"id": "trig_1", "name": "rusty #7 a routine", "created_at": "2026-10-29T11:00:00Z", "enabled": True}


class Handed(unittest.TestCase):
    def setUp(self):
        made = tempfile.TemporaryDirectory()
        self.addCleanup(made.cleanup)
        self.file = Path(made.name, "routines.json")
        self.file.write_text(json.dumps({"triggers": {"data": [TRIGGER]}, "sessions": {"sessions": [SESSION, FIRED]},
                                         "environment_id": ENV_ID}))

    def call(self, arguments: dict):
        return handed.answer(arguments, self.file, NOW)[:2]

    def launch(self):
        fake = Fake()
        fake.call = self.call
        return fake.run("9")

    def test_a_launched_session_lists_as_a_fired_routine_and_a_routines_own_session_not_at_all(self):
        _, listed = self.call({"action": "list"})
        names = [r["name"] for r in listed["data"]]
        self.assertEqual(names, ["rusty #7 a routine", "rusty #8 earlier"])
        session = listed["data"][1]
        self.assertTrue(running.fired(session))
        self.assertEqual(running.issue(session), 8)
        self.assertEqual(routine.environment(listed["data"]), ENV_ID)

    def test_make_coder_queues_its_routine_in_the_outbox_with_the_sessions_environment(self):
        status, _, err = self.launch()
        self.assertEqual(status, 0, err)
        box = json.loads(self.file.read_text())["outbox"]
        self.assertEqual(len(box), 1)
        args = handed.session_args(box[0]["body"])
        self.assertEqual(args["environment_id"], ENV_ID)
        self.assertIn("#9", args["title"])
        self.assertIn("issuecomment-501", args["prompt"])
        self.assertEqual(args["source_url"], "https://github.com/o/r")
        self.assertIn("ToolSearch", args["extra_allowed_tools"])

    def test_a_queued_create_holds_its_slot_on_the_next_listing(self):
        self.launch()
        _, listed = self.call({"action": "list"})
        queued = [r for r in listed["data"] if running.issue(r) == 9]
        self.assertEqual(len(queued), 1)
        self.assertFalse(running.fired(queued[0]))
        self.assertTrue(queued[0]["enabled"])  # `running` reads it as not fired yet: it counts

    def test_what_it_cannot_answer_is_a_501_not_a_silent_success(self):
        self.assertEqual(self.call({"action": "update", "trigger_id": "x", "body": {}})[0], 501)

    def test_remote_answers_from_the_file_when_the_variable_names_one(self):
        with mock.patch.dict(os.environ, {handed.ENV: str(self.file)}):
            status, listed = remote.call({"action": "list"})
        self.assertEqual((status, len(listed["data"])), (200, 2))

    def test_an_unreadable_file_is_a_remote_error(self):
        self.file.write_text("{")
        with mock.patch.dict(os.environ, {handed.ENV: str(self.file)}), self.assertRaises(remote.RemoteError):
            remote.call({"action": "list"})


if __name__ == "__main__":
    unittest.main()
