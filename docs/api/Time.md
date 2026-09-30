## `Time`

Clock accessors, the time-scale control, and the windowed pause / step / resume loop.

| Function | Signature | Returns |
|---|---|---|
| `Time.deltaTime` | `()` | seconds since last frame, **scaled** by `time_scale` |
| `Time.unscaledDeltaTime` | `()` | seconds since last frame, ignoring `time_scale` (raw) |
| `Time.fixedDeltaTime` | `()` | fixed-step seconds (never scaled) |
| `Time.frameCount` | `()` | frames since start |
| `Time.time` | `()` | game seconds since Play started: the sum of the **scaled** `deltaTime`, so it slows with `SetTimeScale` and stops while paused. Shaders read the same clock as `camera.time` (see `Shader.md`). |
| `Time.GetTimeScale` | `()` | current time scale (`1.0` = real time) |
| `Time.SetTimeScale` | `(scale)` — set the global time scale; clamped to `≥ 0`. `0` pauses the sim (`deltaTime` → 0), `0.5` is half-speed slow-mo, `2.0` double-speed. `fixedDeltaTime` is unaffected. Persists across play-mode reset (it's deterministic game state). |
| `Time.Pause` | `()` — freeze the windowed runtime (issue #283): the frame loop stops advancing the sim with wall-clock dt, but the world keeps its exact state and stays inspectable/mutable through the whole API. Rendering continues. Idempotent. |
| `Time.Resume` | `()` — return to normal real-time play **from the current (stepped-to) state** — *not* the pre-play snapshot (that's Stop). Clears any queued steps. Idempotent. |
| `Time.Step` | `(n)` — while paused, advance the world by exactly `n` **fixed-dt** frames, then halt again. The loop pumps `n` ticks at `fixedDeltaTime` (the same fixed step the headless harness uses), so windowed and headless stepping are frame-identical and deterministic. **No-op unless paused** — stepping only makes sense on a frozen world. Repeated calls accumulate. |
| `Time.IsPaused` | `()` | `true` while paused (loop-level halt) |
| `Time.PendingSteps` | `()` | queued fixed-dt steps not yet pumped |

> Only `deltaTime` is scaled — read `unscaledDeltaTime` for anything that must
> keep ticking through a pause or slow-mo (UI, debug cameras). `SetTimeScale` is
> the slow-mo / pause / bullet-time knob; see `tests/time_scale.rs`.

### Pause vs. Step vs. Stop — three distinct operations

These are easy to conflate but do very different things; keep them straight:

- **Pause** (`Time.Pause()`): a **loop-level freeze** that keeps all play-mode state.
  The windowed loop reads the pause flag *before* it ticks and skips the real-time
  advance entirely. The world is frozen but fully inspectable and mutable through the
  API — the right primitive for "freeze → decide → continue the same playthrough."
- **Step** (`Time.Step(n)`): while paused, **advance exactly `n` fixed-dt frames**,
  then halt again. Frame-precise and deterministic (the fixed-dt path), so the same
  `n` steps from the same state always produce the same result.
- **Stop** (`set_playing(false)`, the editor's Stop button / ESC): leaves play mode
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
