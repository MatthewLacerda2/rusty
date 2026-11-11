"""#2063: a draft parked on an open blocker frees its coder's slot.

On 2026-11-11 the showroom coder (#2031) parked PR #2055 as "Draft: blocked by
#2053" and kept counting, so `make coder` refused #2053 itself and #2054 with
four coders really working.
"""

from __future__ import annotations

import unittest

import ghrest
from dispatch import running
from tests.dispatch.test_coder import Fake, busy, drafts
from tests.dispatch.test_running import FIRED, hub, placed, pr


class Blocked(unittest.TestCase):
    def test_a_draft_with_no_open_blocker_counts(self):
        self.assertEqual(running.state(FIRED, hub([pr(2, True)])), ("running", "draft pull request"))

    def test_a_draft_blocked_by_an_open_issue_has_ended_and_is_named(self):
        self.assertEqual(running.state(FIRED, hub([pr(2, True)], blockers={2: [53, 54]})),
                         ("ended", f"{running.BLOCKED} #53, #54"))

    def test_the_listing_notes_it(self):
        counted, noted = running.coders([FIRED], 9, hub([pr(2, True)], blockers={2: [53]}))
        self.assertEqual((counted, noted), ([], [(FIRED, "ended", "draft blocked by #53")]))

    def test_a_closed_blocker_is_no_blocker(self):
        recorded = [{"number": 53, "state": "closed"}, {"number": 54, "state": "open"}]
        self.assertEqual(ghrest.open_blockers(recorded), [54])
        self.assertEqual(placed(FIRED, hub([pr(2, True)], blockers={2: ghrest.open_blockers(recorded[:1])})),
                         "running")

    def test_a_ready_pull_request_is_not_freed_by_a_blocker(self):
        self.assertEqual(running.state(FIRED, hub([pr(2, False)], blockers={2: [53]})),
                         ("running", running.REVIEWING))

    def test_no_pull_request_yet_reads_no_blockers(self):
        def refuse(n):
            raise AssertionError("blockers read without a draft")
        github = hub()
        github.blockers = refuse
        self.assertEqual(placed(FIRED, github), "running")

    def test_unreadable_blockers_are_unknown(self):
        github = hub([pr(2, True)])
        github.blockers = lambda n: None
        self.assertEqual(running.state(FIRED, github), ("unknown", "could not read #2's blockers"))

    def test_make_coder_says_it_and_launches(self):
        parked = {5: [{"number": 77, "state": "open"}, {"number": 76, "state": "closed"}]}
        code, _, err = Fake(routines=busy(1, 2, 3, 4, 5), pulls=drafts(1, 2, 3, 4, 5), blockers=parked).run("9")
        self.assertEqual(code, 0)
        self.assertIn("rusty #5 x: draft blocked by #77; not counted", err)

    def test_the_path_is_the_native_relationship(self):
        self.assertEqual(ghrest.blocked_by("o/r", 7), "repos/o/r/issues/7/dependencies/blocked_by?per_page=100")


if __name__ == "__main__":
    unittest.main()
