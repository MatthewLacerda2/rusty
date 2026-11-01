## `Application`

Unity's `Application`, the subset a shipped game needs (#431): quitting, and the
project's **build settings** — which scene the standalone player boots into, the
product name (the player's window title) and the window mode a first launch opens in.

| Function | Signature | Returns |
|---|---|---|
| `Application.Quit` | `()` | — (a request; see below) |
| `Application.GetStartupScene` / `SetStartupScene` | `()` / `(path)` | `path` (an empty path is an error) |
| `Application.GetProductName` / `SetProductName` | `()` / `(name)` | `name` |
| `Application.GetWindowMode` / `SetWindowMode` | `()` / (`"Windowed"` \| `"Fullscreen"`) | the mode name / `bool` (`false`: unknown name, ignored) |

**`Quit` means what the host says it means.** The call only records the request;
the host acts on it after the tick:

- **Standalone player** — closes the app. The loop's exit flushes `Storage` and the
  video/quality settings first, exactly as closing the window does.
- **Editor** — stops Play (Unity ignores `Quit` in the editor; stopping Play is the
  honest reading of "the game ended"). Outside Play — e.g. typed into the console —
  it is dropped.
- **Headless harness** — ends the run: every later `Harness.Step`/`StepUntil` is a
  no-op, so the scenario's remaining lines observe the state the game quit in, and
  `results.json` records `"quit": true`.

This is what a main-menu "Quit" button calls.

**Build settings** live in the project root's tracked `project.rusty`, under `"build"`
(a missing file or key means the defaults: the seeded `assets/scenes/default.scene`, `"rusty game"`,
`Windowed`; a `project.rusty` the editor creates, as *New Project* does, names the product
after the project's folder instead, Unity's default Product Name). Every way of
starting the engine on a project reads this file at boot, the same way (#1652), and
splits on whether it authors the project or runs it:

- **The editor and the headless session** (`session`, `session-mcp`, the agent's
  path) bind the file: the setters write it, leaving the rest of it alone — they are
  the API twin of **File → Build Settings**.
- **The player, `play` and `editor-capture`** read it but never write it back, so a
  setter only changes the running value. Saved `Video`
settings (a player's own fullscreen choice) win over `SetWindowMode`, which is only
the first-launch default.

A demo replay is the one run that doesn't play under this file: it plays under the
settings recorded in the demo's header, the ones it was recorded with (#1581). A file
that can't be read or parsed leaves the defaults and is reported once, as an error,
in the editor's console and in a session's next reply.

**Graphics option defaults** live in the same file, under `"graphics_options"`: an
object of option name → `bool` (`{ "motion_blur": false }`), every option it omits
allowed. They are what a fresh install's player graphics options start as, and the
same split applies: `Graphics.SetDefaultOption` and the Build Settings window's
**Player graphics defaults** checkboxes write the file in the editor and the
session, and only the running value in the player and `play`. A player's saved choice
(`Graphics.SetOption`) wins over them; see [`Graphics.md`](Graphics.md), *Player
options*. An unknown option name or a non-`bool` value is ignored with a warning in
the console, since the file is the developer's to fix.

**Quality presets** (#1522) live in the same file, under `"quality"`: the game's
named presets, seeded with `Low`, `Medium` and `High` when the file is created, with
the same split (`Graphics.SetQualityPreset` / `RemoveQualityPreset` write the file
in the editor and the session). See [`Graphics.md`](Graphics.md), *Quality presets*.

**Determinism.** `Quit` is a one-way request read by the platform layer after the
tick; the build settings are read at boot. Neither is read by `FixedUpdate`.
