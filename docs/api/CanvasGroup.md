## `CanvasGroup`

Read and tune an entity's `CanvasGroupComponent` (#418) — Unity's `CanvasGroup`.
`Alpha` multiplies every graphic on the entity and below it (nested groups
multiply) — screen fades, greyed-out panels. `Interactable` and `BlocksRaycasts`
are read by the event system (#420). Without a CanvasGroup, `GetAlpha` returns `1`
and the flags `false`; setters are then no-ops.

| Function | Signature | Returns |
|---|---|---|
| `CanvasGroup.GetAlpha` / `SetAlpha` | `(id)` / `(id, alpha)` | subtree opacity, clamped to 0..1 |
| `CanvasGroup.GetInteractable` / `SetInteractable` | `(id)` / `(id, bool)` | whether the subtree's selectables accept input |
| `CanvasGroup.GetBlocksRaycasts` / `SetBlocksRaycasts` | `(id)` / `(id, bool)` | whether the pointer can hit the subtree |
