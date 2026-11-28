#!/usr/bin/env python3
"""`make orchestrator` finds its schedule by the id it stored on the lock, not only in the listing.

The routines listing holds the newest 20, so an older schedule is missed by it;
the id carried in the lock's tree (`schedule.FILE`) is asked for directly.
"""

from __future__ import annotations

import unittest

from orchestrating import carry, schedule, side
from tests.orchestrating.fakes import Remote
from tests.orchestrating.test_orchestrator import LISTED, Routines, run

OLD = {"id": "trig_old", "name": schedule.NAME, "enabled": True}


class StoredId(unittest.TestCase):
    def setUp(self):
        self.git = side.git_in(Remote(self).clone("desk"))

    def stored(self) -> str | None:
        return schedule.stored(carry.shared(self.git, schedule.FILE))

    def test_a_create_stores_its_id_on_the_lock(self):
        self.assertEqual(run(call=Routines(), git=self.git)[0], 0)
        self.assertEqual(self.stored(), "trig_orch")
        self.assertIn(schedule.FILE, carry.SHARED)

    def test_with_the_id_stored_stop_finds_it_whatever_the_listing_holds(self):
        run(call=Routines(), git=self.git)  # stores trig_orch
        call = Routines(listed=LISTED, held=[{"id": "trig_orch", "name": schedule.NAME, "enabled": True}])
        code, out, err = run("--stop", call=call, git=self.git)
        self.assertEqual(code, 0)
        self.assertEqual(call.calls[-1], {"action": "update", "trigger_id": "trig_orch", "body": {"enabled": False}})
        self.assertNotIn("may miss", err)
        self.assertIsNone(self.stored())  # cleared once disabled

    def test_and_a_second_create_is_refused_by_it(self):
        run(call=Routines(), git=self.git)
        call = Routines(listed=LISTED, held=[{"id": "trig_orch", "name": schedule.NAME, "enabled": True}])
        code, _, err = run(call=call, git=self.git)
        self.assertEqual(code, 1)
        self.assertIn("enabled already (trig_orch)", err)
        self.assertNotIn("create", [c["action"] for c in call.calls])

    def test_a_stored_schedule_disabled_by_hand_lets_a_new_one_be_made(self):
        run(call=Routines(), git=self.git)
        call = Routines(listed=LISTED, held=[{"id": "trig_orch", "name": schedule.NAME, "enabled": False}])
        self.assertEqual(run(call=call, git=self.git)[0], 0)

    def test_with_no_id_stored_it_falls_back_to_the_listing_with_a_warning(self):
        call = Routines(listed=[*LISTED, OLD])
        code, _, err = run("--stop", call=call, git=self.git)
        self.assertEqual(code, 0)
        self.assertEqual(call.calls[-1]["trigger_id"], "trig_old")
        self.assertIn("holds the newest only and may miss one", err)

    def test_an_unreadable_record_is_no_id(self):
        for text in (None, "", "{", "[]", '{"id": 3}', '{"id": ""}'):
            self.assertIsNone(schedule.stored(text), text)
        self.assertEqual(schedule.stored(schedule.record("trig_x")), "trig_x")


if __name__ == "__main__":
    unittest.main()
