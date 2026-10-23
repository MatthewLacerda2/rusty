#!/usr/bin/env python3
"""`launcher.py [--remove]` — rusty in the desktop's app search (#1315).

`make launcher` builds the dev editor and runs this, so "Rusty" opens from the
Linux app grid and search, or from Spotlight and Launchpad on macOS, like any
installed app. The operator looked for it there on 2026-10-23 and found nothing.

What it installs, in the user's home and nowhere else:

- **Linux:** `rusty.desktop` in `$XDG_DATA_HOME/applications` (default
  `~/.local/share`), and the logo from `engine/icon/` in the user's `hicolor`
  icon theme. The editor's window names itself `rusty` (`shell::boot`), which is
  how the desktop matches the open window to this entry and gives it the logo:
  Wayland takes a window's icon from its `.desktop` file only.
- **macOS:** `~/Applications/Rusty.app`, whose executable is a script that starts
  the checkout's binary, with an `.icns` built by `sips` and `iconutil` (both ship
  with macOS); `mdimport` then has Spotlight index it at once.

Both start `<checkout>/target/debug/rusty` **from the checkout**: the engine reads
`engine/shaders` and `engine/scripts` relative to the working directory, and a
launcher starts apps from `$HOME`. The entry points at the binary, not a copy, so
every later `cargo build --features dev --bin rusty` is what it opens.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

CHECKOUT = Path(__file__).resolve().parents[2]
ICONS = CHECKOUT / "engine" / "icon"
# The app id `shell::boot::editor_app_id` gives the window, and so the entry's name.
APP_ID = "rusty"
LINUX_ICON_SIDES = (16, 32, 256, 512, 1024)
# iconutil's iconset: each point size at 1x and 2x, the largest 512 pt.
MAC_ICON_POINTS = (16, 32, 128, 256, 512)


def binary(root: Path) -> Path:
    return root / "target" / "debug" / "rusty"


def _desktop_string(value: str) -> str:
    """A `.desktop` string value: backslash and control characters escaped."""
    return value.replace("\\", "\\\\").replace("\n", "\\n").replace("\t", "\\t")


def _exec_arg(value: str) -> str:
    """One `Exec` argument, double-quoted with the spec's escapes (`%` doubled, as a
    field code would read it), then escaped as a string."""
    quoted = "".join("\\" + c if c in '"`$\\' else c for c in value).replace("%", "%%")
    return _desktop_string(f'"{quoted}"')


def desktop_entry(root: Path) -> str:
    """The `rusty.desktop` that starts the editor from `root`."""
    return "\n".join(
        [
            "[Desktop Entry]",
            "Type=Application",
            "Name=Rusty",
            "GenericName=Game Engine",
            f"Comment={_desktop_string(f'The rusty editor, from {root}')}",
            f"Exec={_exec_arg(str(binary(root)))}",
            f"Path={_desktop_string(str(root))}",
            f"Icon={APP_ID}",
            "Terminal=false",
            "Categories=Development;Game;",
            "Keywords=game;engine;editor;",
            f"StartupWMClass={APP_ID}",
            "",
        ]
    )


def _sh(value: str) -> str:
    return "'" + value.replace("'", "'\\''") + "'"


def mac_runner(root: Path) -> str:
    """The bundle's executable: start the checkout's editor from the checkout."""
    return f'#!/bin/sh\ncd {_sh(str(root))} && exec {_sh(str(binary(root)))} "$@"\n'


def info_plist() -> str:
    """The bundle's `Info.plist`."""
    keys = {
        "CFBundleDisplayName": "Rusty",
        "CFBundleExecutable": APP_ID,
        "CFBundleIconFile": APP_ID,
        "CFBundleIdentifier": "io.github.matthewlacerda2.rusty",
        "CFBundleName": "Rusty",
        "CFBundlePackageType": "APPL",
    }
    body = "".join(f"\t<key>{k}</key>\n\t<string>{v}</string>\n" for k, v in keys.items())
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" '
        '"http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0">\n<dict>\n'
        f"{body}\t<key>NSHighResolutionCapable</key>\n\t<true/>\n"
        "</dict>\n</plist>\n"
    )


def linux_paths(env: dict[str, str], home: Path) -> tuple[Path, Path]:
    """`(the .desktop file, the hicolor theme folder)` under the user's data home."""
    data = Path(env.get("XDG_DATA_HOME") or home / ".local" / "share")
    return data / "applications" / f"{APP_ID}.desktop", data / "icons" / "hicolor"


def mac_bundle(home: Path) -> Path:
    return home / "Applications" / "Rusty.app"


def _quietly(*argv: str) -> None:
    """Run a desktop's optional refresh tool; its absence is not an error."""
    if shutil.which(argv[0]):
        subprocess.run(argv, check=False, capture_output=True)


def install_linux(root: Path, entry: Path, theme: Path) -> None:
    for side in LINUX_ICON_SIDES:
        icon = theme / f"{side}x{side}" / "apps" / f"{APP_ID}.png"
        icon.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ICONS / f"rusty-{side}.png", icon)
    entry.parent.mkdir(parents=True, exist_ok=True)
    entry.write_text(desktop_entry(root))
    _quietly("update-desktop-database", str(entry.parent))
    _quietly("gtk-update-icon-cache", "--quiet", "--ignore-theme-index", str(theme))


def remove_linux(entry: Path, theme: Path) -> None:
    entry.unlink(missing_ok=True)
    for side in LINUX_ICON_SIDES:
        (theme / f"{side}x{side}" / "apps" / f"{APP_ID}.png").unlink(missing_ok=True)
    _quietly("update-desktop-database", str(entry.parent))


def _icns(out: Path) -> None:
    """Build the bundle's `.icns` from the 1024 px logo with `sips` and `iconutil`."""
    with tempfile.TemporaryDirectory() as tmp:
        iconset = Path(tmp) / f"{APP_ID}.iconset"
        iconset.mkdir()
        for points in MAC_ICON_POINTS:
            for scale, suffix in ((1, ""), (2, "@2x")):
                px = str(points * scale)
                name = iconset / f"icon_{points}x{points}{suffix}.png"
                source = str(ICONS / "rusty-1024.png")
                subprocess.run(
                    ["sips", "-z", px, px, source, "--out", str(name)],
                    check=True,
                    capture_output=True,
                )
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(out)], check=True)


def install_mac(root: Path, bundle: Path) -> None:
    shutil.rmtree(bundle, ignore_errors=True)
    contents = bundle / "Contents"
    (contents / "MacOS").mkdir(parents=True)
    (contents / "Resources").mkdir()
    (contents / "Info.plist").write_text(info_plist())
    runner = contents / "MacOS" / APP_ID
    runner.write_text(mac_runner(root))
    runner.chmod(0o755)
    _icns(contents / "Resources" / f"{APP_ID}.icns")
    _quietly("mdimport", str(bundle))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--remove", action="store_true", help="take the entry away")
    args = parser.parse_args(argv)
    home = Path.home()
    if sys.platform == "darwin":
        bundle = mac_bundle(home)
        if args.remove:
            shutil.rmtree(bundle, ignore_errors=True)
        else:
            install_mac(CHECKOUT, bundle)
        where = bundle
    elif sys.platform.startswith("linux"):
        entry, theme = linux_paths(dict(os.environ), home)
        if args.remove:
            remove_linux(entry, theme)
        else:
            install_linux(CHECKOUT, entry, theme)
        where = entry
    else:
        print(f"launcher: {sys.platform} is not supported (macOS and Linux are)", file=sys.stderr)
        return 1
    if args.remove:
        print(f"launcher: removed {where}")
    else:
        if not binary(CHECKOUT).exists():
            print(f"launcher: {binary(CHECKOUT)} is not built yet", file=sys.stderr)
        print(f"launcher: installed {where}, starting {binary(CHECKOUT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
