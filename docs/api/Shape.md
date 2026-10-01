## `Shape`

Read and tune an entity's `ShapeComponent` (#425) — a **texture-free UI graphic
drawn from a signed distance field**, crisp at any scale: rounded or cut-corner
panels, ellipses, rings and arcs, solid or dashed lines. It fills the entity's
laid-out rect (rotate the `RectTransform` to aim a line). Add or remove it with
`Scene.AddComponent(id, "Shape")` / `RemoveComponent` (adding also adds a
`RectTransform`). Lengths are reference units; colours are **display-space RGBA
with straight alpha**, each `0..1`. Getters return a neutral default (zeros,
`"None"`, `false`, `nil`) without a Shape; setters are then no-ops. Multi-value
setters need every number (an error names the count). How shapes draw, and a
"Cyberpunk panel" recipe, are in [`docs/ui.md`](../ui.md#look-shapes-gradients-effects-and-blend-modes).

| Function | Signature | Returns |
|---|---|---|
| `Shape.GetKind` / `SetKind` | `(id)` / `(id, name)` | `"Rect"`, `"Ellipse"`, `"Ring"` or `"Line"` |
| `Shape.GetCorner` / `SetCorner` | `(id)` / `(id, name)` | `Rect`: `"Round"` (quarter circles) or `"Chamfer"` (45° cuts) |
| `Shape.GetRadius` / `SetRadius` | `(id)` / `(id, tl, tr, br, bl)` | `Rect`: per-corner size (each ≥ 0), CSS order |
| `Shape.GetInnerRadius` / `SetInnerRadius` | `(id)` / `(id, r)` | `Ring`: the hole's radius (0: a disc or pie) |
| `Shape.GetArc` / `SetArc` | `(id)` / `(id, start, end)` | `Ring`: degrees clockwise from 12 o'clock; a sweep ≥ 360 is the full ring |
| `Shape.GetThickness` / `SetThickness` | `(id)` / `(id, t)` | `Line`: thickness |
| `Shape.GetDash` / `SetDash` | `(id)` / `(id, dash, gap)` | `Line`: dash and gap lengths (dash 0: solid) |
| `Shape.GetColor` / `SetColor` | `(id)` / `(id, r, g, b, a)` | fill colour (ignored while a gradient is set) |
| `Shape.GetGradient` / `SetGradient` | `(id)` / `(id, gradient)` | the fill gradient table (below), or `nil`; `nil` clears it |
| `Shape.GetBorder` / `SetBorder` | `(id)` / `(id, width, r, g, b, a)` | the outline band drawn inside the edge (width 0: none) |
| `Shape.GetShadow` / `SetShadow` | `(id)` / `(id, dx, dy, blur, r, g, b, a)` | drop shadow: offset (y up), edge softness, colour (alpha 0: off) |
| `Shape.GetGlow` / `SetGlow` | `(id)` / `(id, size, intensity, r, g, b, a)` | outer glow: reach past the edge, brightness multiplier, colour (size or alpha 0: off) |
| `Shape.GetBlend` / `SetBlend` | `(id)` / `(id, name)` | `"Normal"`, `"Additive"`, `"Multiply"` or `"Screen"` |
| `Shape.GetRaycastTarget` / `SetRaycastTarget` | `(id)` / `(id, bool)` | whether the pointer can hit it (its rect) |

Name setters are case-insensitive; an unrecognized name is ignored.

**Gradients** (shared with `Image.SetGradient`) are a table — or the same document
as a JSON string:

```lua
Shape.SetGradient(id, {
  kind = "Linear",          -- or "Radial"
  angle = 90,               -- Linear: degrees counter-clockwise from +x (90 = bottom → top)
  center = { 0.5, 0.5 },    -- Radial: centre as a fraction of the rect
  radius = 0.5,             -- Radial: where the last stop sits (0.5 = each edge's middle)
  stops = {                 -- 2 to 4, any order (kept sorted), t in 0..1
    { t = 0, color = { 0.0, 0.9, 1.0, 1.0 } },
    { t = 1, color = { 0.0, 0.9, 1.0, 0.0 } },
  },
})
```

Omitted keys take their defaults; fewer than two stops clears the gradient, extra
stops past four are dropped. `GetGradient` returns the table in this shape.
