"""`memory_floor.py` reads the runner's available memory on both platforms and
keeps the lowest seen (#2363); a floor it cannot read informs, never fails."""

from __future__ import annotations

import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import memory_floor

MEMINFO = "MemTotal:       16384000 kB\nMemFree:         1000000 kB\nMemAvailable:    8192000 kB\n"
VM_STAT = """Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                               1000.
Pages active:                             9000.
Pages inactive:                           2000.
Pages speculative:                         500.
Pages purgeable:                           500.
"Translation faults":                   123456.
"""


class Read(unittest.TestCase):
    def test_linux_takes_mem_available_not_mem_free(self):
        self.assertEqual(memory_floor.linux(MEMINFO), (8192000, 16384000))

    def test_mac_counts_free_inactive_and_speculative_not_active_or_purgeable(self):
        available, total = memory_floor.mac(VM_STAT, str(7 * 1024 ** 3))
        self.assertEqual(available, 3500 * 16384 // 1024)
        self.assertEqual(total, 7 * 1024 ** 2)

    def test_a_meminfo_without_mem_available_raises_for_the_watch_to_record(self):
        with self.assertRaises(KeyError):
            memory_floor.linux("MemTotal: 1 kB\n")


class Floor(unittest.TestCase):
    def test_the_floor_keeps_the_lowest_and_counts_the_samples(self):
        floor = None
        for now in [(5, 10), (3, 10), (7, 10)]:
            floor = memory_floor.lower(floor, now)
        self.assertEqual(floor, (3, 10, 3))

    def test_the_line_names_the_floor_its_share_and_the_samples(self):
        line = memory_floor.line(f"{2 * 1024 ** 2} {8 * 1024 ** 2} 600 0\n")
        self.assertEqual(line, "memory: at least 2.0 GiB of 8.0 GiB available (25%) "
                               "over 600 one-second samples during the tests")
        self.assertIn("600 one-second samples during the tests, 2 unreadable",
                      memory_floor.line(f"{2 * 1024 ** 2} {8 * 1024 ** 2} 600 2\n"))

    def test_an_unreadable_floor_says_so(self):
        self.assertIn("no floor", memory_floor.line("unreadable: no vm_stat\n"))

    def test_a_missing_file_informs_and_passes(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = memory_floor.main(["memory_floor.py", "report", "/nonexistent/floor.txt"])
        self.assertEqual(code, 0)
        self.assertIn("memory: no floor", out.getvalue())

    def test_report_reads_what_a_watch_wrote(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "floor.txt")
            path.write_text("1048576 4194304 2 0\n")
            out = io.StringIO()
            with redirect_stdout(out):
                memory_floor.main(["memory_floor.py", "report", str(path)])
        self.assertIn("1.0 GiB of 4.0 GiB", out.getvalue())


class Watch(unittest.TestCase):
    def watch(self, reads: list) -> str:
        def read():
            got = reads.pop(0)
            if isinstance(got, Exception):
                raise got
            return got
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "floor.txt")
            memory_floor.watch(path, 0, read, len(reads))
            self.assertEqual([p.name for p in Path(tmp).iterdir()], ["floor.txt"])
            return path.read_text()

    def test_a_failed_sample_keeps_the_floor_and_the_watch_goes_on(self):
        self.assertEqual(self.watch([(5, 10), OSError("vm_stat"), (3, 10)]), "3 10 2 1\n")

    def test_with_no_sample_read_the_file_says_why(self):
        self.assertEqual(self.watch([OSError("no vm_stat")]), "unreadable: no vm_stat\n")


if __name__ == "__main__":
    unittest.main()
