## `Editor` — **dev builds only**

Mirrors Unity's `EditorApplication.isPlaying` (#1360): enter and leave Play from a
script, the console or an agent, so a play-test needs no one to press Play. And
Unity's `Undo` (#1631): step the edit-mode history back and forth. Registered
only under the `dev` Cargo feature, like [`Debug`](Debug.md); a shipped game (the
`player` build) has no `Editor` namespace. A **standalone** run in a dev build (a
dev-built `player`, the `play` harness) has no edit mode to switch to or from, as
Unity's `EditorApplication` doesn't exist in a player: there `EnterPlay` and
`ExitPlay` refuse, return `false` and warn in the console.

| Function | Signature | Returns |
|---|---|---|
| `Editor.EnterPlay` | `()` | `true` when it asked to enter Play; `false` (a no-op) when already playing or already asked to (by a script, or the Play button this frame), or in a standalone run |
| `Editor.ExitPlay` | `()` | `true` when it asked to leave Play; `false` (a no-op) in edit mode or when already asked to, or in a standalone run |
| `Editor.IsPlaying` | `()` | `true` while the world is in Play |
| `Editor.OpenDemo` | `(path)` | `true` when it asked the editor to open the demo at `path` on the Game view's timeline; `false` (a no-op, warned) outside the editor window |
| `Editor.SeekDemo` | `(tick)` | `true` when it asked the open demo's replay to go to `tick`; `false` outside the editor window |
| `Editor.PauseDemo` | `([paused])` | `true` when it asked to pause (default) or, with `false`, play the open demo; `false` outside the editor window |
| `Editor.DemoTick` | `()` | the open demo's current tick, or `nil` with none open |
| `Editor.DemoLength` | `()` | how many ticks the open demo holds, or `nil` with none open |
| `Editor.Undo` | `()` | the name of the step it undid; `nil` when there was none, or in Play |
| `Editor.Redo` | `()` | the name of the step it redid; `nil` when there was none, or in Play |
| `Editor.UndoName` | `()` | the name of the step `Undo` would revert, or `nil` |
| `Editor.RedoName` | `()` | the name of the step `Redo` would re-apply, or `nil` |

### Undo and Redo

Unity's `Undo` (#1631): the same history as **Ctrl+Z** / **Ctrl+Shift+Z** (Cmd on
macOS; Ctrl+Y redoes too) and **Edit ▸ Undo / Redo**. Every edit-mode change is
recorded, whoever made it: an Inspector field, the gizmo, the hierarchy, a menu, and
every API call from the console, a script line or an agent.

- **One command line is one step**, named after the line (its first 60 characters):
  `Scene.SetName(5, "Door")` then `Editor.Undo()` puts the old name back. Lines sent
  together are still one step each.
- **A gesture by hand is one step**: a whole slider or gizmo drag, from press to
  release, named after what it touched (`Edit Door`, `Create Cube`, `Destroy Cube`,
  `Edit 3 objects`, `Scene Settings`).
- **What comes back**: each touched entity whole (its components, name, parent,
  place in the hierarchy and its id: a destroyed entity returns under the same id),
  and the scene level (Scene Settings, ambient light, fog, layers, the material
  library, probes, the bake's settings). Selection is not part of a step.
- **Edit mode only.** Nothing is recorded in Play, the history is cleared when Play
  starts (Stop already restores Play's changes) and when a scene is loaded, and in
  Play `Undo` / `Redo` do nothing and return `nil`. A new edit clears Redo. The
  history keeps the last 200 steps.
- An undo is an edit: the scene has unsaved changes after it, as after any other.

```lua
Scene.SetName(5, "Door")
Editor.UndoName()            -- 'Scene.SetName(5, "Door")'
Editor.Undo()                -- the old name is back
Editor.Redo()                -- "Door" again
```

### Play

**The same transition as the Play/Stop button.** A call only *requests* it; the
engine takes the request at the next frame boundary, exactly as it takes the button.
So on the line that calls `Editor.EnterPlay()`, `Editor.IsPlaying()` is still `false`;
in the next reply's command it is `true`. That holds **per reply**: lines sent
together, before the first reply, can all run in one frame, before the transition.
Requests made in one frame do not stack: a later one replaces an earlier one, so
`EnterPlay()` then `ExitPlay()` in the same frame leaves edit mode as it was. Leaving Play restores the edit-mode snapshot, as
Stop does: edit-mode changes made before Play come back, play-mode ones are gone.
A gameplay script may call `Editor.ExitPlay()` too (Unity allows it); Play ends after
that frame.

**Globals do not survive the switch.** Each Play and each Stop builds a fresh Lua VM
([`index.md`](index.md#driving-a-headless-session)), so the command after the switch
runs in the new one: set what it needs again.

**The play-test loop** (the attached window and the headless session alike), one
command per reply:

```lua
Time.Pause()                 -- first: Play starts with no wall-clock frame
Editor.EnterPlay()           -- the next command runs in Play
Input.Press("W")             -- drive the game
Time.Step(30)                -- advance 30 fixed-dt frames
Debug.Snapshot()             -- read the result (or `snapshot` over MCP)
Editor.ExitPlay()            -- back to the authored scene
Time.Resume()                -- last: the next Play (the button too) runs live
```

Pause **before** entering: the window otherwise runs real-time frames between
`EnterPlay` and the first `Time.Step`, and the run stops being the same every time.
Paused, the window enters as the headless session and the harness do. A pause
outlives Stop, so **resume after** leaving, or the operator's next Play is frozen.

The headless session never advances on its own: after each command it runs one
paused frame, which takes a Play/Stop request and runs the steps `Time.Step` queued.
`Time.Step` needs `Time.Pause()` first there, as in the window. An error that frame
logs (a script's `Awake`/`Start`/`Update`, a Lua init failure) comes back with the
reply, as engine warnings do: `Editor.EnterPlay()` answers `true` for the request,
and the warnings say whether the scripts it started ran.

## The demo timeline

A demo (`saved/demos/*.demo`, [`docs/testing.md`](../testing.md) *Demos*) opens in
the **editor window** and plays on the Game view with a timeline: play/pause, a scrub
bar with the index's `Debug.Mark`s on it, the tick readout and a close button. The
calls above are those controls (Unity's Profiler/Timeline scrubbing a capture), and
take effect at the next frame, like `EnterPlay`; only the editor window has a
timeline, so a headless session or a standalone run refuses them.

- `Editor.OpenDemo(path)` loads the demo's scene, replays it on the editor's world
  and stops on tick 0. Refused while Play is on (stop it first) and for a demo with
  no ticks or whose scene file is gone; the reason goes to the console. The edit
  scene you had (unsaved edits included) comes back on close.
- The replay plays at the speed it was recorded: each tick takes the `dt` it was
  recorded with (a 144 Hz recording plays at 144 ticks a second), and it holds on
  the last tick, where Play (the button or `Editor.PauseDemo(false)`) starts it
  over. Scrubbing back
  replays from the start (`Harness.SeekDemo`'s rule), so a drag over a long demo
  costs more the further back it goes.
- **Read-only.** The Game view takes none of the editor's input; the replay's recorded
  input is the only input. Edits made by hand in the Inspector or hierarchy during
  playback are not blocked and make the replay diverge, as in any Play.
- The Game view keeps the size the demo recorded (the UI lays out on it), drawn
  letterboxed in the panel; a demo recorded headless plays on its video resolution.
- **Your files are left alone.** The replay starts from the save data, settings,
  quality preset, seed and clock the demo recorded, but on a store bound to no file:
  nothing it does reaches `saved/storage.json`. Closing gives the editor back its
  own, as they were.
- Closing, by the timeline's button, by Stop or by quitting the editor, leaves the
  replay, puts the edit scene back and resumes recording Play sessions. There is no `CloseDemo`: it is
  `Editor.ExitPlay()`.
