## `Selectable`

Read and tune an entity's `SelectableComponent` (#420) — Unity's `Selectable`, the
base every widget builds on: it makes the entity something the pointer and the
keyboard focus interact with, and shows its **state** — `"Normal"`,
`"Highlighted"` (hovered), `"Pressed"`, `"Selected"` (focused) or `"Disabled"` —
through a transition on its target graphic. Adding one also adds a
`RectTransform`. Getters return a neutral default (`false`, `nil`, zeros) without
a Selectable; setters are then no-ops. States, directions (`"Up"`, `"Down"`,
`"Left"`, `"Right"`) and enum values are names, case-insensitive; an unknown name
is ignored. The model is in [`docs/ui.md`](../ui.md).

| Function | Signature | Returns |
|---|---|---|
| `Selectable.GetInteractable` / `SetInteractable` | `(id)` / `(id, bool)` | its own interactable flag |
| `Selectable.IsInteractable` | `(id)` | whether it accepts input now — its flag and every `CanvasGroup.interactable` above it |
| `Selectable.GetState` | `(id)` | the state name it shows this tick, or `nil` without a Selectable |
| `Selectable.GetTransition` / `SetTransition` | `(id)` / `(id, name)` | `"None"`, `"ColorTint"` or `"SpriteSwap"` |
| `Selectable.GetTargetGraphic` / `SetTargetGraphic` | `(id)` / `(id, targetId)` | the entity whose Image / Text shows the state; `nil` is the Selectable's own entity |
| `Selectable.GetColor` / `SetColor` | `(id, state)` / `(id, state, r, g, b, [a])` | `ColorTint` colour for `state`, display-space RGBA 0..1 |
| `Selectable.GetFadeDuration` / `SetFadeDuration` | `(id)` / `(id, seconds)` | `ColorTint` fade, in unscaled seconds (≥ 0) |
| `Selectable.GetSprite` / `SetSprite` | `(id, state)` / `(id, state, path)` | `SpriteSwap` texture for `state`, or `nil` (the Image's own; `"Normal"` always is) |
| `Selectable.GetNavigation` / `SetNavigation` | `(id)` / `(id, name)` | `"None"`, `"Automatic"` or `"Explicit"` |
| `Selectable.GetSelectOn` / `SetSelectOn` | `(id, dir)` / `(id, dir, targetId)` | the `Explicit` target for `dir`, or `nil` |
