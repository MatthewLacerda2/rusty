"""#2452: a run log's first edit, gates and end, read from recorded condensed lines."""

from __future__ import annotations

import unittest
from datetime import datetime, timezone

from dispatch import runlog
from tests.dispatch.runlog_fixtures import NEWER, OLDER, STILL


def at(clock: str) -> datetime:
    return datetime.fromisoformat(f"2026-11-20T{clock}+00:00")


class FirstEdit(unittest.TestCase):
    def test_an_edit_tool_is_an_edit(self):
        for name in ("Edit", "Write", "MultiEdit", "NotebookEdit"):
            self.assertTrue(runlog.edits(name, "{}"), name)
        self.assertFalse(runlog.edits("Read", '{"file_path":"a"}'))

    def test_a_bash_that_writes_a_file_or_commits_is_an_edit(self):
        for shell in ("git commit -m x", "git -C /w commit -am x", "sed -i 's/a/b/' src/a.rs",
                      "cat > src/a.rs <<'EOF'", "echo x >> docs/a.md", "printf x | tee src/b.rs"):
            self.assertTrue(runlog.writes(shell), shell)

    def test_a_bash_that_reads_or_writes_only_scratch_is_not(self):
        for shell in ("cat src/a.rs", "make test > target/test.log 2>&1", "grep x a 2>/dev/null",
                      "cargo build &> /tmp/b", "ls | head", "git status", "echo hi > out.log"):
            self.assertFalse(runlog.writes(shell), shell)

    def test_a_bash_that_moves_patches_or_copies_into_the_tree_is_an_edit(self):
        for shell in ("git apply /tmp/p.diff", "git mv a.rs b.rs", "cd /w && git rm -q a.rs", "patch -p1 < x",
                      "git -C /w apply x", "cp /tmp/a.rs src/a.rs", "mv -f a b/c.rs", "cargo fmt &> src/x.rs",
                      "echo x &>> docs/a.md"):
            self.assertTrue(runlog.writes(shell), shell)

    def test_a_merge_a_copy_into_scratch_and_a_format_patch_are_not(self):
        for shell in ("git merge origin/main --no-edit", "git fetch origin main && git merge origin/main",
                      "cp src/a.rs /tmp/a.rs", "mv target/a target/b", "git format-patch -1", "cp -r src",
                      "make test &> target/test.log"):
            self.assertFalse(runlog.writes(shell), shell)

    def test_a_command_cut_short_by_the_log_is_still_read(self):
        cut = '{"command":"cat > src/a.rs <<\'EOF\'\\nfn a() {}\\n… [+900 chars]'
        self.assertTrue(runlog.edits("Bash", cut))


class Gate(unittest.TestCase):
    def test_cargo_and_make_are_gates_anywhere_in_a_compound_command(self):
        for shell in ("make gates", "cd /w && make clippy", "CARGO_BUILD_JOBS=2 cargo nextest run",
                      "timeout 600 make test > target/t.log", "git status; cargo check", "(cargo doc)",
                      'RUSTFLAGS="-D warnings" cargo clippy', "A='x y' B=1 make gates"):
            self.assertTrue(runlog.gate(shell), shell)

    def test_a_word_that_only_mentions_them_is_not(self):
        for shell in ("grep make Makefile", "git commit -m 'make it cargo'", "cat Cargo.toml", "makes"):
            self.assertFalse(runlog.gate(shell), shell)


class Read(unittest.TestCase):
    def setUp(self):
        self.times = runlog.read(runlog.lines([NEWER, OLDER]))

    def test_pages_are_put_back_in_order_and_untimed_lines_dropped(self):
        found = runlog.lines([NEWER, OLDER])
        self.assertEqual(found[0][0], datetime(2026, 11, 20, 2, 27, 20, 100000, tzinfo=timezone.utc))
        self.assertEqual([a for a, _ in found], sorted(a for a, _ in found))
        self.assertFalse(any("continued" in text for _, text in found))

    def test_the_start_the_first_edit_and_the_end(self):
        self.assertEqual(self.times.init, at("02:27:58"))
        self.assertEqual(self.times.first_edit, at("02:29:22"))  # the Edit, not the `cat` before it
        self.assertEqual(self.times.end, at("03:06:15"))
        self.assertEqual(self.times.turns, 46)
        self.assertFalse(self.times.running)

    def test_parallel_gates_count_once(self):
        # 02:30:00–02:34:00 and 02:30:00–02:37:00 overlap: seven minutes, not eleven
        self.assertEqual(self.times.gate_seconds(), 420)
        self.assertEqual(self.times.gate_seconds(stop=self.times.first_edit), 0)

    def test_a_turn_after_the_last_result_is_still_running(self):
        times = runlog.read(runlog.lines([STILL]))
        self.assertTrue(times.running)
        self.assertEqual(times.gate_seconds(), 0)  # the open gate ends at the last line, its own start

    def test_a_log_with_no_result_is_running(self):
        self.assertTrue(runlog.read(runlog.lines([OLDER])).running)
        self.assertTrue(runlog.read([]).running)


if __name__ == "__main__":
    unittest.main()
