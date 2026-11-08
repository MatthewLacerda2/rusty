## `Time`

Clock accessors, the time-scale control, and the windowed pause / step / resume loop.

| Function | Signature | Returns |
|---|---|---|
| `Time.deltaTime` | `()` | seconds this tick advances the sim, **scaled** by `time_scale`: in Play, `fixedDeltaTime × time_scale` (see *What runs at which rate*) |
| `Time.unscaledDeltaTime` | `()` | the same, ignoring `time_scale`: `fixedDeltaTime` in Play; an edit-mode frame's wall-clock seconds outside it |
| `Time.fixedDeltaTime` | `()` | fixed-step seconds (never scaled) |
| `Time.frameCount` | `()` | ticks since Play started (a windowed frame may run none or several) |
| `Time.time` | `()` | game seconds since Play started: the sum of the **scaled** `deltaTime`, so it slows with `SetTimeScale` and stops while paused. Shaders read the same clock as `camera.time` (see `Shader.md`). |
| `Time.unscaledTime` | `()` | seconds since Play started, ignoring `time_scale`: the sum of `unscaledDeltaTime`, so it keeps running at time scale 0. Custom UI shaders animate on this clock (see `Shader.md`, *The `ui` pass*), so a paused menu still flickers. |
| `Time.GetTimeScale` | `()` | current time scale (`1.0` = real time) |
| `Time.SetTimeScale` | `(scale)` — set the global time scale; clamped to `≥ 0`. `0` pauses the sim (`deltaTime` → 0), `0.5` is half-speed slow-mo, `2.0` double-speed. `fixedDeltaTime` is unaffected. Persists across play-mode reset (it's deterministic game state). |
| `Time.Pause` | `()` — freeze the windowed runtime (issue #283): the frame loop stops advancing the sim with wall-clock dt, but the world keeps its exact state and stays inspectable/mutable through the whole API. Rendering continues. Idempotent. |
| `Time.Resume` | `()` — return to normal real-time play **from the current (stepped-to) state** — *not* the pre-play snapshot (that's Stop). Clears any queued steps. Idempotent. |
| `Time.Step` | `(n)` — while paused, advance the world by exactly `n` **fixed-dt** frames, then halt again. The loop pumps `n` ticks at `fixedDeltaTime` (the same fixed step the headless harness uses), so windowed and headless stepping are frame-identical and deterministic. **No-op unless paused** — stepping only makes sense on a frozen world. Repeated calls accumulate. |
| `Time.IsPaused` | `()` | `true` while paused (loop-level halt) |
| `Time.PendingSteps` | `()` | queued fixed-dt steps not yet pumped |

### What runs at which rate

The whole sim runs on the **fixed tick**, `fixedDeltaTime` = 1/60 s, in the editor's
Play, the `player` binary, the headless harness and `Time.Step` alike (#1877): the
windowed loop adds each frame's wall-clock seconds to an accumulator and runs every
whole tick now owed, so a 144 Hz display runs a tick on about every second or third
frame, and a 30 Hz one two ticks per frame. A frame adds at most 1/3 s (Unity's
`Time.maximumDeltaTime`), so a hitch runs at most 20 ticks and the game slows rather
than spiralling. Entering or leaving Play runs one tick in that frame. Edit mode is not
on the tick: there each frame advances by its own wall-clock time, as it always has.

The accumulator adds **unscaled** time: `SetTimeScale` scales each tick's `deltaTime`
(`fixedDeltaTime × time_scale`), not how many ticks run (Unity counts fewer
`FixedUpdate`s at a lower scale instead). So scripts keep ticking at scale 0, which is
what a `SetTimeScale(0)` pause menu needs, since rusty's `Update` runs inside the tick
too, and a windowed run equals the harness's tick for tick at every scale.

Every reader of the tick's delta takes the **tick's** `dt`: scripts' `Update`,
`LateUpdate` and timers, physics, animation, particles, trails, decals, navigation
agents and obstacles, UI sway, the audio mix's clock and game time (`Time.time`,
`camera.time`). Nothing in the sim reads a frame's wall-clock time. Input written
between ticks reaches the next tick only: a frame that runs no tick leaves a press
pending, and of several ticks in one frame only the first sees the frame's key edges,
mouse delta and text.

**Drawing blends between ticks.** Each frame draws every moving mesh and the main
camera between their last two tick poses, by how far the frame is into the next tick
(Unity's `Rigidbody.interpolation`, for everything that moves), so motion is smooth at
any refresh rate and what is drawn lags the sim by at most one tick. An entity that
moved more than 2 m or turned more than 90° in one tick jumped (a respawn, a script
setting its Transform) and is drawn where it landed. Paused, stepped or in edit mode,
everything is drawn exactly where it is. Skinned bones, particles, ribbons, decals,
lights and stacked or render-texture cameras are drawn at the tick. The sim never
reads a blended pose: `Transform` and every API return the tick's.

> Only `deltaTime` is scaled — read `unscaledDeltaTime` for anything that must
> keep ticking through a pause or slow-mo (UI, debug cameras). `SetTimeScale` is
> the slow-mo / pause / bullet-time knob; see `tests/time_scale.rs`.

### Pause vs. Step vs. Stop — three distinct operations

These are easy to conflate but do very different things; keep them straight:

- **Pause** (`Time.Pause()`): a **loop-level freeze** that keeps all play-mode state.
  The windowed loop reads the pause flag *before* it ticks and skips the real-time
  advance entirely. The world is frozen but fully inspectable and mutable through the
  API — the right primitive for "freeze → decide → continue the same playthrough."
  The headless session never advances on its own: after each command it runs one
  paused frame, so there too the world moves only by `Time.Step` ([`Editor`](Editor.md)).
- **Step** (`Time.Step(n)`): while paused, **advance exactly `n` fixed-dt frames**,
  then halt again. Frame-precise and deterministic (the fixed-dt path), so the same
  `n` steps from the same state always produce the same result.
- **Stop** (`Editor.ExitPlay()`, the editor's Stop button / Ctrl+P, Cmd+P on macOS): leaves play mode
  and **restores the edit snapshot, discarding all play-mode state**. Stop is a
  *reset*, not a pause — use it to return to the authored scene, never to "continue
  later."

**Pause vs. `SetTimeScale(0)`.** Both freeze the sim, but they're different
mechanisms and compose rather than conflict. `SetTimeScale` scales `deltaTime`
*while the sim runs* — it's the in-game slow-mo / bullet-time knob, and a
`SetTimeScale(0)` world is still being ticked every frame (by a zero dt). `Pause` is
a harder, loop-level halt: the windowed loop doesn't tick the sim at all (no schedule
run), which is what makes frame-precise `Step` meaningful. They layer: a paused world
stepped via `Step` still scales each fixed step by the current `time_scale`. Reach
for `SetTimeScale` for in-game time effects; reach for `Pause`/`Step`/`Resume` for the
agent's "freeze the playtest, advance N frames, resume" loop. A game's **pause menu**
is therefore `SetTimeScale(0)` plus UI animated with `unscaledDeltaTime` — never
`Pause`, which would halt the menu's own scripts (see [`docs/ui.md`](../ui.md)). All of these are
reachable identically from the in-app console and the external command channel
(issue #282), since they're plain `Time` verbs on the one shared evaluator.
