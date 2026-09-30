## `Light`

Read and tune an entity's `LightComponent` — colour, intensity, range and type.
Every setter maps onto a field the renderer reads when it packs the lighting
uniform, so a change takes effect on the next frame. A light has no per-component
"active" flag: it is gated by its owning entity's `active` (a `Scene` concern), so
this surface exposes the light's own data and nothing more. Getters return a
neutral default (`(1,1,1)` colour, `0` scalars, `"None"` type) when the entity has
no light.

| Function | Signature | Returns |
|---|---|---|
| `Light.GetColor` | `(id)` | `r, g, b` (linear) |
| `Light.SetColor` | `(id, r, g, b)` | — |
| `Light.GetIntensity` / `SetIntensity` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetRange` / `SetRange` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetType` / `SetType` | `(id)` / `(id, name)` | `"Ambient"` / `"Directional"` / `"Point"` / `"Spotlight"` |

`SetType` is case-insensitive; an unrecognized name is ignored (the current type
is kept).
