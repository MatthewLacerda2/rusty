## `Editor` — **dev builds only**

Mirrors Unity's `EditorApplication.isPlaying` (#1360): enter and leave Play from a
script, the console or an agent, so a play-test needs no one to press Play. Registered
only under the `dev` Cargo feature, like [`Debug`](Debug.md); a shipped game (the
`player` build) has no `Editor` namespace.

| Function | Signature | Returns |
|---|---|---|
| `Editor.EnterPlay` | `()` | `true` when it asked to enter Play; `false` (a no-op) when already playing, or already asked to |
| `Editor.ExitPlay` | `()` | `true` when it asked to leave Play; `false` (a no-op) in edit mode, or when already asked to |
| `Editor.IsPlaying` | `()` | `true` while the world is in Play |

**The same transition as the Play/Stop button.** A call only *requests* it; the
engine takes the request at the next frame boundary, exactly as it takes the button.
So on the line that calls `Editor.EnterPlay()`, `Editor.IsPlaying()` is still `false`;
on the next command it is `true`. Leaving Play restores the edit-mode snapshot, as
Stop does: edit-mode changes made before Play come back, play-mode ones are gone.
A gameplay script may call `Editor.ExitPlay()` too (Unity allows it); Play ends after
that frame.

**Globals do not survive the switch.** Each Play and each Stop builds a fresh Lua VM
([`index.md`](index.md#driving-a-headless-session)), so the command after the switch
runs in the new one: set what it needs again.

**The play-test loop** (the attached window and the headless session alike):

```lua
Editor.EnterPlay()           -- the next command runs in Play
Time.Pause()                 -- freeze, so steps are exact
Input.Press("W")             -- drive the game
Time.Step(30)                -- advance 30 fixed-dt frames
Debug.Snapshot()             -- read the result (or `snapshot` over MCP)
Editor.ExitPlay()            -- back to the authored scene
```

The headless session never advances on its own: after each command it runs one
paused frame, which takes a Play/Stop request and runs the steps `Time.Step` queued.
`Time.Step` needs `Time.Pause()` first there, as in the window.
