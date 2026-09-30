## `Image`

Read and tune an entity's `ImageComponent` (#418) — the UI's rectangle graphic,
Unity's `Image`. It fills the entity's laid-out rect with a tint colour, optionally
multiplied by a texture. Colours are **display-space (sRGB) RGBA with straight
alpha**, each `0..1` — a 50% alpha looks exactly as it does in an image editor.
Getters return a neutral default (zeros, `"None"`, `false`, `nil`) when the entity
has no Image; setters are then no-ops. Adding an Image also adds a `RectTransform`.
How each type draws is in [`docs/ui.md`](../ui.md).

| Function | Signature | Returns |
|---|---|---|
| `Image.GetColor` / `SetColor` | `(id)` / `(id, r, g, b, a)` | `r, g, b, a` (each clamped to 0..1) |
| `Image.GetTexture` / `SetTexture` | `(id)` / `(id, path)` | texture path, or `nil` for a solid colour (`nil` / `""` clears it) |
| `Image.GetType` / `SetType` | `(id)` / `(id, name)` | `"Simple"`, `"Sliced"`, `"Tiled"` or `"Filled"` |
| `Image.GetBorder` / `SetBorder` | `(id)` / `(id, l, b, r, t)` | `Sliced` 9-slice borders in texels (each ≥ 0) |
| `Image.GetFillMethod` / `SetFillMethod` | `(id)` / `(id, name)` | `"Horizontal"`, `"Vertical"` or `"Radial360"` |
| `Image.GetFillOrigin` / `SetFillOrigin` | `(id)` / `(id, name)` | `"Left"`, `"Right"`, `"Bottom"` or `"Top"` — fitted to the method (`Bottom`↔`Left`, `Top`↔`Right`) |
| `Image.GetFillAmount` / `SetFillAmount` | `(id)` / `(id, amount)` | the visible fraction, clamped to 0..1 |
| `Image.GetFillClockwise` / `SetFillClockwise` | `(id)` / `(id, bool)` | `Radial360` sweep direction |
| `Image.GetPreserveAspect` / `SetPreserveAspect` | `(id)` / `(id, bool)` | `Simple`: fit the texture's aspect inside the rect |
| `Image.GetRaycastTarget` / `SetRaycastTarget` | `(id)` / `(id, bool)` | whether the pointer can hit it |

Name setters are case-insensitive; an unrecognized name is ignored. A health bar
is `SetType(id, "Filled")` + `SetFillAmount(id, hp / max)`; a cooldown ring is the
same with `SetFillMethod(id, "Radial360")`.
