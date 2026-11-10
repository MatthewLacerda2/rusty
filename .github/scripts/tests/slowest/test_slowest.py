"""`slowest_tests.py` reads nextest's JUnit report (#1976): the longest first,
capped, a case without a readable time skipped, and a missing report no failure."""

from __future__ import annotations

import io
import unittest
from contextlib import redirect_stdout

import slowest_tests

REPORT = """<?xml version="1.0" encoding="UTF-8"?>
<testsuites><testsuite name="rusty">
<testcase name="a::fast" classname="rusty" time="0.010"/>
<testcase name="a::slow" classname="rusty" time="96.500"/>
<testcase name="a::middle" classname="rusty" time="4.200"><failure/></testcase>
<testcase name="a::untimed" classname="rusty"/>
</testsuite><testsuite name="main"><testcase name="gpu::frame" classname="main" time="12.0"/></testsuite>
</testsuites>"""


class Slowest(unittest.TestCase):
    def test_the_longest_come_first_and_the_list_is_capped(self):
        self.assertEqual(slowest_tests.slowest(REPORT, 3),
                         [(96.5, "rusty a::slow"), (12.0, "main gpu::frame"), (4.2, "rusty a::middle")])

    def test_a_case_without_a_time_is_skipped(self):
        names = [name for _, name in slowest_tests.slowest(REPORT, 10)]
        self.assertEqual(len(names), 4)
        self.assertNotIn("rusty a::untimed", names)

    def test_a_missing_report_informs_and_passes(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = slowest_tests.main(["slowest_tests.py", "/nonexistent/junit.xml"])
        self.assertEqual(code, 0)
        self.assertIn("no timings", out.getvalue())


if __name__ == "__main__":
    unittest.main()
