## `Camera`

| Function | Signature | Returns |
|---|---|---|
| `Camera.GetPosition` | `()` | `x, y, z` |
| `Camera.SetPosition` | `(x, y, z)` | — |
| `Camera.GetForward` | `()` | `x, y, z` (unit look direction) |
| `Camera.GetRight` | `()` | `x, y, z` (unit right vector) |
| `Camera.GetYaw` / `SetYaw` | `()` / `(yaw)` | degrees |
| `Camera.GetPitch` / `SetPitch` | `()` / `(pitch)` | degrees (clamped ±89) |
| `Camera.GetFov` / `SetFov` | `()` / `(fov)` | degrees (clamped 1–179) |
