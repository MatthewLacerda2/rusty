## `Transform`

Per-entity position / rotation (Euler degrees) / scale. Mutators re-sync the
entity's collider.

| Function | Signature | Returns |
|---|---|---|
| `Transform.GetPosition` | `(id)` | `x, y, z` |
| `Transform.SetPosition` | `(id, x, y, z)` | — |
| `Transform.GetRotation` | `(id)` | `x, y, z` (Euler degrees) |
| `Transform.SetRotation` | `(id, x, y, z)` | — |
| `Transform.GetScale` | `(id)` | `x, y, z` |
| `Transform.SetScale` | `(id, x, y, z)` | — |
| `Transform.MoveTowards` | `(id, tx, ty, tz, step)` | — (steps toward target, snaps within `step`) |
