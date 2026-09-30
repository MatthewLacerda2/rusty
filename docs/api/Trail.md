## `Trail`

Drive an entity's `TrailComponent` (#441) — Unity's `TrailRenderer`: a ribbon that
follows the entity through the world over a time window. Rocket smoke, bullet
tracers, grenade arcs in flight, a melee swipe.

**How it records.** Once per fixed tick in Play, right after `LateUpdate`, the
trail samples the entity's **world** position (so a trail on a child follows its
parent) and ages its points on the same scaled `dt`. The newest point, the
**head**, follows the entity; once it is `min_vertex_distance` past the point
before it, it is kept and a new head starts. A point older than `time` seconds
drops off the tail. With emitting off nothing new is recorded and the trail ages
away. The recording is sim state, so a headless replay records the identical
trail. It starts empty on entering Play and is never saved; an inactive entity's
trail is frozen and hidden. Call `Clear` after a teleport, or the trail streaks
across the jump.

**How it looks.** Width and colour run along the trail's length, `t = 0` at the
head (the entity) and `t = 1` at the tail. The default tapers from `0.1` to `0` and
fades from opaque to clear. The ribbon always faces the camera. It is transparent:
drawn after the transparent solids, it fades into the scene fog like a particle,
takes no ambient occlusion, is unlit and **casts no shadow**. Blend `"Alpha"`
(smoke) or `"Additive"` (tracers — an HDR colour above 1 blooms).

Without a Trail, getters return a neutral default (`false`, `0`, `nil`, an empty
list) and setters do nothing. Add one with `Scene.AddComponent(id, "Trail")`.

| Function | Signature | Returns |
|---|---|---|
| `Trail.IsEmitting` / `SetEmitting` | `(id)` / `(id, on)` | whether new points are recorded |
| `Trail.GetTime` / `SetTime` | `(id)` / `(id, seconds)` | how long a point lives (clamped `≥ 0`) |
| `Trail.GetMinVertexDistance` / `SetMinVertexDistance` | `(id)` / `(id, distance)` | how far the head moves before a point is kept (clamped `≥ 0`) |
| `Trail.Clear` | `(id)` | — forgets every recorded point |
| `Trail.GetPositionCount` | `(id)` | the number of recorded points |
| `Trail.GetPositions` | `(id)` | a list of `{x, y, z}`, oldest first, world space |
| `Trail.GetWidth` / `SetWidth` | `(id, t?)` / `(id, start, end?)` | the width at `t` (default `0`); set a straight taper, or a constant without `end` |
| `Trail.SetWidthCurve` | `(id, {{t, width}, …})` | — any width curve; `t` clamped to `[0, 1]`, widths `≥ 0` |
| `Trail.GetColor` / `SetColor` | `(id, t?)` / `(id, r, g, b, a?)` | `r, g, b, a` at `t`; set one colour everywhere (`a` defaults to `1`) |
| `Trail.SetColors` | `(id, r1, g1, b1, a1, r2, g2, b2, a2)` | — blend from the head colour to the tail colour |
| `Trail.GetTexture` / `SetTexture` | `(id)` / `(id, path)` | the texture path or `nil`; `nil` or `""` goes back to plain white |
| `Trail.GetTextureMode` / `SetTextureMode` | `(id)` / `(id, mode)` | `"Stretch"` (one copy over the length) or `"Tile"` (repeats every world unit) |
| `Trail.GetBlend` / `SetBlend` | `(id)` / `(id, blend)` | `"Alpha"` or `"Additive"` (unknown names are ignored) |

```lua
-- A bright tracer that lives a tenth of a second.
Scene.AddComponent(bullet, "Trail")
Trail.SetTime(bullet, 0.1)
Trail.SetBlend(bullet, "Additive")
Trail.SetColors(bullet, 4, 3, 1, 1,  4, 2, 0.5, 0)
Trail.SetWidth(bullet, 0.03, 0)
```
