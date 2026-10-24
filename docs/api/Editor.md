## `Editor` — **dev builds only**

Mirrors Unity's `EditorApplication.isPlaying` (#1360): enter and leave Play from a
script, the console or an agent, so a play-test needs no one to press Play. Registered
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
