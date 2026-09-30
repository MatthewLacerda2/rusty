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

**Build settings** live in the tracked `project/build_settings.json` (a missing file
means the defaults: the seeded `project/scenes/default.scene`, `"rusty game"`,
`Windowed`). In the editor the setters write that file — they are the API twin of
**File → Build Settings**. In the player and the harness the settings are read but
never written back, so a setter only changes the running value. Saved `Video`
settings (a player's own fullscreen choice) win over `SetWindowMode`, which is only
the first-launch default.

**Determinism.** `Quit` is a one-way request read by the platform layer after the
tick; the build settings are read at boot. Neither is read by `FixedUpdate`.
