"""#2667: the coders' cap lives on the lock, so a cap set on either side is the one every side runs under.

Must: a cap shared while the other side holds the lock reaches that side's next take and survives its
renewals and release; the holder's renewal that races the cap's push still renews.
Must not: a holder's own older cap file overwrite the shared one; the cap's push change the lease.
"""

from __future__ import annotations

import unittest

from orchestrating import carry, lease
from tests.orchestrating.fakes import CLOUD, LOCAL, OTHER_CLOUD, Remote

CAP = carry.SHARED[0]


def cap_in(side) -> str | None:
    path = side.directory / CAP
    return path.read_text() if path.exists() else None


def write(side, text: str) -> None:
    side.directory.mkdir(parents=True, exist_ok=True)
    (side.directory / CAP).write_text(text)


class Shared(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.cloud, self.local = self.remote.side("a", CLOUD), self.remote.side("b", LOCAL)

    def share(self, side, text: str) -> None:
        write(side, text)
        carry.share(side.git, CAP, text, "cap: set", self.remote.clock.now)

    def test_a_cap_set_here_while_a_fire_holds_the_lock_reaches_the_next_fire(self):
        self.cloud.take()
        write(self.cloud, '{"cap": 5}\n')  # the fire's copy, older than the operator's
        self.share(self.local, '{"cap": 3}\n')
        self.assertEqual(lease.read(self.local.git).holder, CLOUD)  # the lease is as it was
        self.assertIsNone(self.cloud.renew())
        self.assertIsNone(self.cloud.release())
        nxt = self.remote.side("c", OTHER_CLOUD)
        self.assertIn(CAP, nxt.take()[1])
        self.assertEqual(cap_in(nxt), '{"cap": 3}\n')

    def test_this_machine_taking_back_its_own_release_reads_the_cap_a_fire_set(self):
        self.local.take()
        self.local.release()
        write(self.local, '{"cap": 4}\n')
        self.share(self.cloud, '{"cap": 2}\n')
        self.assertEqual(self.local.take()[1], [CAP])
        self.assertEqual(cap_in(self.local), '{"cap": 2}\n')

    def test_a_cap_on_a_lock_never_taken_starts_it_free(self):
        self.share(self.local, '{"cap": 3}\n')
        found = lease.read(self.cloud.git)
        self.assertIsNone(found.holder)
        self.assertIsNone(self.cloud.take()[0])
        self.assertEqual(cap_in(self.cloud), '{"cap": 3}\n')

    def test_a_renewal_that_lost_the_race_to_the_cap_reads_the_tip_and_renews(self):
        self.cloud.take()
        racing, pushes = self.cloud.git, []

        def git(*args, stdin=None):
            if args[0] == "push" and not pushes:  # the cap lands between the renewal's read and its push
                pushes.append(args)
                self.share(self.local, '{"cap": 1}\n')
            return racing(*args, stdin=stdin)
        self.cloud.git = git
        self.assertIsNone(self.cloud.renew())
        self.assertEqual(lease.read(self.local.git).holder, CLOUD)
        self.cloud.release()
        nxt = self.remote.side("c", OTHER_CLOUD)
        nxt.take()
        self.assertEqual(cap_in(nxt), '{"cap": 1}\n')


if __name__ == "__main__":
    unittest.main()
