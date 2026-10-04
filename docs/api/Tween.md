## `Tween`

Animate a component property to a target over time on an easing curve (#424) —
a menu sliding in, a hit marker popping, a damage vignette fading, a door
swinging, a light flickering — without hand-rolling a lerp in `Update`.

Every tween is **owned by the entity it animates** (`id`) and gets an integer
**handle** from the same handle space as [`Timer`](Timer.md). Starting one on a
missing or inactive entity, or on a component the entity lacks, is an error.

| Function | Signature | Returns |
|---|---|---|
| `Tween.To` | `(id, property, target, duration [, opts])` — drive `property` from its current value to `target` over `duration` seconds (`>= 0`). | handle |
| `Tween.Sequence` | `(steps)` — play tweens one after another; see below. | sequence handle |
| `Tween.Kill` | `(handle)` — stop a tween where it is (its value stays; no `on_complete`), or every tween of a sequence handle. Not for timers: `Timer.*` handles are untouched. | `true` if anything was playing |
| `Tween.KillAll` | `(id)` — stop every tween animating the entity. | — |
| `Tween.IsPlaying` | `(handle)` | `true` while that tween (or any tween of that sequence, including one still in its delay) has not finished or been killed |

**Properties** are `"Component.field"` paths. Only these (and the runtime shader
params below) are animatable — a typo is an error listing them. A one-number property takes a number; the others take
a table of exactly that many numbers:

- `{x, y, z}`: `Transform.position`, `Transform.scale`, and `Transform.rotation`
  (euler degrees, each angle interpolated on its own, like `Transform.SetRotation`)
- `{x, y}`: `RectTransform.anchored_position`, `RectTransform.size_delta`,
  `RectTransform.pivot`, `RectTransform.anchor_min`, `RectTransform.anchor_max`
- `{r, g, b, a}`: `Image.color`, `Text.color`
- `{r, g, b}`: `Light.color`
- a number: `CanvasGroup.alpha`, `Image.fill_amount`, `Text.font_size`,
  `Light.intensity`, `Light.range`, `Camera.fov`, `AudioSource.volume`

**Runtime shader params** (#661) tween too, as the namespace word followed by the
param's name — `block.param`, or `block.index.param` when the block repeats — so
a value as wide as the param (a number, or `{…}` of 2–4):

| Path | Drives | Same as |
|---|---|---|
| `"Material.<name>"`, e.g. `"Material.hit_flash.amount"` | entity `id`'s **own override** — one enemy flashes, not every one sharing its material | `Material.SetShaderParam(id, name, v)` |
| `"UI.<name>"`, e.g. `"UI.dissolve.amount"` | every graphic of `id` drawn with its ui shader | `UI.SetShaderParam(id, name, v)` |
| `"Graphics.<name>"`, e.g. `"Graphics.damage_vignette.intensity"` | the post-processing volume **on entity `id`** | `Graphics.SetPostParam(name, v)` on that volume |

The shader's layout is resolved once, at `Tween.To`: a param the shader bakes, a
typo, an entity with no material / graphic / volume, or no shader named yet is an
error there (listing the runtime params), never a silent no-op. Each step writes
through the setter's own op. A tween ends quietly if the component goes, or — for
`UI.` — the graphic is switched to another shader. UI shaders animate on unscaled
time, so `unscaled = true` is the natural pairing for a `UI.` tween.

```lua
-- On hit: flash this enemy and let it fade.
Tween.KillAll(enemy)
Tween.To(enemy, "Material.hit_flash.amount", 0, 0.15, { from = 1, ease = "quad_out" })

-- On damage: spike the vignette, then let it settle.
Tween.To(volume, "Graphics.damage_vignette.intensity", 0, 0.8, { from = 1 })
```

Each value is written through the same authoring op as the component's own
setter (`CanvasGroup.SetAlpha`, `Image.SetColor`, …), so it is clamped the same
way: an overshooting ease can push a position past its target, but never an
alpha past 1. A moved `Transform` drags its collider along, as `Transform.Set*`
does.

**`opts`** (a table; an unknown key is an error):

| Key | Default | Meaning |
|---|---|---|
| `ease` | `"linear"` | The curve. `linear`, `ease_in` / `ease_out` / `ease_in_out` (quad), or a family — `quad`, `cubic`, `quart`, `expo`, `sine`, `back`, `elastic`, `bounce` — with `_in`, `_out` or `_in_out`: `"quad_out"`, `"back_out"`, `"elastic_in_out"`. snake_case, like zimmer's `Sound` easings. `back` and `elastic` overshoot on purpose. |
| `delay` | `0` | Seconds before it starts. The start value is read when the delay ends. |
| `from` | current value | Start here instead. Written **at once**, so a delayed fade-in is already hidden while it waits. |
| `loops` | `1` | Cycles to play; `-1` loops until killed (needs `duration > 0`). |
| `yoyo` | `false` | Every other cycle plays backwards (an even count ends back at the start). |
| `unscaled` | `false` | Tick on unscaled time, so it keeps moving under `Time.SetTimeScale(0)` — a pause menu's own animations. |
| `on_complete` | — | `fn(id)`, called once when the last cycle lands on its end value. |

```lua
-- A pause menu that slides in and fades up while the game is frozen.
Time.SetTimeScale(0)
Tween.To(menu, "CanvasGroup.alpha", 1, 0.2, { from = 0, unscaled = true })
Tween.To(menu, "RectTransform.anchored_position", {0, 0}, 0.35,
    { from = {0, -80}, ease = "back_out", unscaled = true })

-- A hit marker that pops and fades.
Tween.To(marker, "RectTransform.size_delta", {48, 48}, 0.12,
    { from = {24, 24}, ease = "back_out", yoyo = true, loops = 2 })
```

**Sequences.** `Tween.Sequence(steps)` lays tweens out on one timeline. A number
is a gap in seconds; a table is a tween written as `{id, property, target,
duration, opts...}` — `Tween.To`'s arguments with its options as named fields —
that starts once everything before it has ended. With `join = true` it starts
together with the previous tween instead (plus its own `delay`), which is how a
staggered entrance is written. Every step is validated before any starts; an
endless (`loops = -1`) step is an error. `Kill` / `IsPlaying` on the returned
handle cover the whole sequence; each step keeps its own `on_complete`.

```lua
Tween.Sequence{
    {title, "CanvasGroup.alpha", 1, 0.3, from = 0},
    0.1,
    {play, "RectTransform.anchored_position", {0, 0}, 0.25, from = {-400, 0}, ease = "quad_out"},
    {options, "RectTransform.anchored_position", {0, -60}, 0.25, from = {-400, -60}, ease = "quad_out", join = true, delay = 0.05},
    {quit, "RectTransform.anchored_position", {0, -120}, 0.25, from = {-400, -120}, ease = "quad_out", join = true, delay = 0.05},
}
```

**When it runs — deterministic, in the fixed step.** Tweens are jobs of the
timer scheduler: they advance in the same timer phase as `Timer` invokes and
coroutines (after every `Update`, before physics and `LateUpdate`), in the same
ascending `(entity id, handle)` order, on the tick's own `dt` — never the wall
clock — so a replay writes every tweened value on the same tick. The same
one-tick rule holds: a tween started on tick `N` first moves on tick `N + 1`
(after its delay). Two tweens on one property both write; the later handle
wins — `KillAll` first to replace one. `on_complete` runs inside that phase, in
that order.

**Lifetime** is a timer's: tweens stop for good (no `on_complete`) when their
entity is **deactivated**, **destroyed**, or unloaded by `Scene.Load` (a
`Scene.DontDestroyOnLoad` survivor keeps its tweens), or when the tweened
component is removed. They are runtime state, **not scene data**: every Play
starts with none, and Stop restores the edit snapshot. `Timer.Cancel` and
`Timer.IsPending` also accept a tween handle (one handle space); `Tween.Kill`
touches only tweens.

**Errors.** An error inside `on_complete` is logged as `[Lua Error] Tween
on_complete on entity N failed: …`.
