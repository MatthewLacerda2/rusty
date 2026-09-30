## `Particles`

Drive an entity's particle emitter (`ParticleEmitterComponent`). `Emit`/`Burst`
spawn through the emitter's own seeded path, so scripted emission stays
deterministic in a headless replay. `Emit`/`Burst` return the number of particles
actually spawned (the `max_particles` cap may swallow some).

| Function | Signature | Returns |
|---|---|---|
| `Particles.Emit` | `(id, count)` | spawned count |
| `Particles.Burst` | `(id)` | spawned count (fires the configured `burst_count`) |
| `Particles.SetActive` | `(id, active)` | — |
| `Particles.SetRate` | `(id, rate)` | — (continuous particles/sec, clamped ≥ 0) |
| `Particles.IsActive` | `(id)` | `bool` |
| `Particles.GetCount` | `(id)` | live particle count |
| `Particles.Clear` | `(id)` | — (despawns all live particles) |
| `Particles.SetShape` | `(id, kind, opts?)` | `bool` (false for an unknown kind) |
| `Particles.SetDirection` | `(id, x, y, z)` | — (the shape's axis / launch direction) |
| `Particles.SetLifetime` | `(id, min, max?)` | — (seconds, clamped ≥ 0) |
| `Particles.SetSpeed` | `(id, min, max?)` | — |
| `Particles.SetSize` | `(id, min, max?)` | — (start size, clamped ≥ 0) |
| `Particles.SetColor` | `(id, r, g, b, a?)` | — (start tint; `a` defaults to 1) |
| `Particles.SetSubEmitter` | `(id, trigger, target?)` | `bool` (false for an unknown trigger) |

**Ranges.** `SetLifetime`/`SetSpeed`/`SetSize` take `min, max`: each particle
draws its start value uniformly between them from the emitter's seeded stream.
Omit `max` for a constant.

**Shapes.** `kind` is one of `Point`, `Sphere`, `Hemisphere`, `Box`, `Cone`,
`Circle` (case-insensitive). `opts` is a table: `radius` (default 1), `angle`
(cone half-angle in degrees, default 25), `x`/`y`/`z` (box size, default 1) and
`surface` (`true` emits from the surface / edge instead of the volume). Shapes are
oriented by the emitter's direction, so aiming a hemisphere of sparks along an
impact normal is one `SetDirection` call:

```lua
Particles.SetShape(sparks, "Hemisphere", { radius = 0.05 })
Particles.SetDirection(sparks, nx, ny, nz)
Particles.Burst(sparks)
```

**Sub-emitters.** `trigger` is `"birth"`, `"death"` or `"collision"`; `target` is
another entity with an emitter (usually an inactive child), or `nil` to clear.
When one of this emitter's particles is born, dies or hits a collider (the
emitter's collision response must not be `None`), the target fires its
`burst_count` at that particle's position, inheriting the fraction of its velocity
set by the emitter's `inherit_velocity`.

Over-life curves (size, colour gradient, drag, rotation speed) are authored in the
inspector or the scene file, not scripted.
