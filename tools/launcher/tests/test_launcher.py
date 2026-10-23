#!/usr/bin/env python3
"""#1315: the launcher's files start the checkout's editor from the checkout.

The installers write only under the home they are given, so these run against
a temporary one, with the desktop's refresh tools stubbed; the macOS `.icns`
step needs `sips` and is left out here.
"""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from unittest import mock
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "launcher.py"
_spec = importlib.util.spec_from_file_location("launcher", SCRIPT)
launcher = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(launcher)


def keys(entry: str) -> dict[str, str]:
    return dict(line.split("=", 1) for line in entry.splitlines() if "=" in line)


class DesktopEntry(unittest.TestCase):
    def test_starts_the_checkout_binary_from_the_checkout(self):
        entry = keys(launcher.desktop_entry(Path("/home/me/rusty")))
        self.assertEqual(entry["Exec"], '"/home/me/rusty/target/debug/rusty"')
        self.assertEqual(entry["Path"], "/home/me/rusty")
        self.assertEqual(entry["Icon"], "rusty")
        # The window's app id (`shell::boot::editor_app_id`) matches the entry.
        self.assertEqual(entry["StartupWMClass"], "rusty")

    def test_quotes_what_exec_would_read_as_syntax(self):
        entry = keys(launcher.desktop_entry(Path('/w/a b$"%\\c')))
        self.assertEqual(entry["Exec"], '"/w/a b\\\\$\\\\"%%\\\\\\\\c/target/debug/rusty"')
        self.assertEqual(entry["Path"], '/w/a b$"%\\\\c')


class Mac(unittest.TestCase):
    def test_the_runner_cds_into_the_checkout(self):
        runner = launcher.mac_runner(Path("/Users/me/it's"))
        self.assertIn("cd '/Users/me/it'\\''s' && exec '/Users/me/it'\\''s/target/debug/rusty'", runner)

    def test_the_plist_names_the_executable_and_icon(self):
        plist = launcher.info_plist()
        self.assertIn("<key>CFBundleExecutable</key>\n\t<string>rusty</string>", plist)
        self.assertIn("<key>CFBundleIconFile</key>\n\t<string>rusty</string>", plist)


class Linux(unittest.TestCase):
    def test_paths_follow_xdg_data_home(self):
        entry, theme = launcher.linux_paths({"XDG_DATA_HOME": "/x"}, Path("/h"))
        self.assertEqual(entry, Path("/x/applications/rusty.desktop"))
        self.assertEqual(theme, Path("/x/icons/hicolor"))
        entry, _ = launcher.linux_paths({}, Path("/h"))
        self.assertEqual(entry, Path("/h/.local/share/applications/rusty.desktop"))

    # The desktop's refresh tools are the host's, and write caches of their own.
    @mock.patch.object(launcher, "_quietly")
    def test_install_then_remove_leaves_nothing(self, _refresh):
        with tempfile.TemporaryDirectory() as tmp:
            entry, theme = launcher.linux_paths({}, Path(tmp))
            launcher.install_linux(Path("/r"), entry, theme)
            self.assertIn("Exec=", entry.read_text())
            self.assertTrue((theme / "256x256" / "apps" / "rusty.png").is_file())
            launcher.remove_linux(entry, theme)
            left = [p for p in Path(tmp).rglob("*") if p.is_file()]
            self.assertEqual(left, [])


if __name__ == "__main__":
    unittest.main()
