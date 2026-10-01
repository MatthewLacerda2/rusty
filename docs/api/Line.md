## `Line`

Drive an entity's `LineComponent` (#441) — Unity's `LineRenderer`: a ribbon through
a list of points you set. Laser sights, a grenade's trajectory preview, taser
beams, debug lines in a shipped game.

**Points.** Indexed from `0`. In **world space** (the default) they are where they
say; with world space off they are in the entity's **local** space, so the line
moves, turns and scales with the entity — a laser sight parented to a gun needs
setting only once. With loop on, the last point joins back to the first. A point
that is not a finite number is dropped; a line holds at most 4096 points.

**How it looks.** Width and colour run along the line's length, `t = 0` at the
first point and `t = 1` at the last. The default is `0.1` wide and white, from
`(0, 0, 0)` to `(0, 0, 1)`. The ribbon always faces the camera. It is
transparent: drawn after the transparent solids, it fades into the scene fog like a
particle, takes no ambient occlusion, is unlit and **casts no shadow**. Blend
`"Alpha"` or `"Additive"` (an HDR colour above 1 blooms). An inactive entity's line
is hidden.

Without a Line, getters return a neutral default (`false`, `0`, `nil`, an empty
list) and setters do nothing. Add one with `Scene.AddComponent(id, "Line")`.

| Function | Signature | Returns |
|---|---|---|
| `Line.GetPositions` / `SetPositions` | `(id)` / `(id, {{x, y, z}, …})` | every point as a list of `{x, y, z}`; set replaces them all |
| `Line.GetPosition` / `SetPosition` | `(id, index)` / `(id, index, x, y, z)` | one point, `x, y, z` (zeros out of range); setting out of range does nothing |
| `Line.GetPositionCount` / `SetPositionCount` | `(id)` / `(id, count)` | the number of points; growing repeats the last point |
| `Line.GetUseWorldSpace` / `SetUseWorldSpace` | `(id)` / `(id, on)` | whether the points are world space (else local) |
| `Line.GetLoop` / `SetLoop` | `(id)` / `(id, on)` | whether the last point joins the first |
| `Line.GetWidth` / `SetWidth` | `(id, t?)` / `(id, start, end?)` | the width at `t` (default `0`); set a straight taper, or a constant without `end` |
| `Line.SetWidthCurve` | `(id, {{t, width}, …})` | — any width curve; `t` clamped to `[0, 1]`, widths `≥ 0` |
| `Line.GetColor` / `SetColor` | `(id, t?)` / `(id, r, g, b, a?)` | `r, g, b, a` at `t`; set one colour everywhere (`a` defaults to `1`) |
| `Line.SetColors` | `(id, r1, g1, b1, a1, r2, g2, b2, a2)` | — blend from the first point's colour to the last's |
| `Line.GetTexture` / `SetTexture` | `(id)` / `(id, path)` | the texture path or `nil`; `nil` or `""` goes back to plain white |
| `Line.GetTextureMode` / `SetTextureMode` | `(id)` / `(id, mode)` | `"Stretch"` (one copy over the length) or `"Tile"` (repeats every world unit) |
| `Line.GetBlend` / `SetBlend` | `(id)` / `(id, blend)` | `"Alpha"` or `"Additive"` (unknown names are ignored) |

```lua
-- A red laser sight on the gun, pointing down its barrel (local -Z).
Scene.AddComponent(gun, "Line")
Line.SetUseWorldSpace(gun, false)
Line.SetPositions(gun, {{0, 0, 0}, {0, 0, -50}})
Line.SetWidth(gun, 0.01)
Line.SetColors(gun, 3, 0, 0, 1,  3, 0, 0, 0)
Line.SetBlend(gun, "Additive")
```
