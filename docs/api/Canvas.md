## `Canvas`

Read and tune an entity's `CanvasComponent` — the root of an in-game UI (#417).
A canvas spans the whole screen; its descendants carrying a `RectTransform` are
laid out inside it. The scaler is Unity's *Scale With Screen Size*: layout runs in
**reference units** (default 1920×1080) and the scale factor maps them onto the
live screen. The model — coordinates, draw order, determinism — is in
[`docs/ui.md`](../ui.md). Getters return a neutral default (`0`, `"None"`, `nil`) when
the entity has no canvas.

| Function | Signature | Returns |
|---|---|---|
| `Canvas.GetRenderMode` / `SetRenderMode` | `(id)` / `(id, name)` | `"ScreenSpaceOverlay"` (the only mode until #429) |
| `Canvas.GetSortOrder` / `SetSortOrder` | `(id)` / `(id, order)` | integer; higher draws on top and is hit first |
| `Canvas.GetReferenceResolution` / `SetReferenceResolution` | `(id)` / `(id, w, h)` | `w, h` (each clamped ≥ 1) |
| `Canvas.GetMatchWidthOrHeight` / `SetMatchWidthOrHeight` | `(id)` / `(id, value)` | `0` = match width … `1` = match height (clamped) |
| `Canvas.GetScaleFactor` | `(id)` | screen pixels per reference unit on the current screen, or `nil` |

`SetRenderMode` is case-insensitive; an unrecognized name is ignored.
