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
| `Particles.SetRenderMode` | `(id, mode)` | `bool` (false for an unknown mode) |
| `Particles.GetRenderMode` | `(id)` | mode name, or `nil` without an emitter |
| `Particles.SetStretch` | `(id, length_scale, speed_scale?)` | — (each clamped ≥ 0) |
| `Particles.SetMesh` | `(id, mesh?, material?)` | — (`nil` clears) |
| `Particles.SetFlipbook` | `(id, columns, rows, opts?)` | — |
| `Particles.SetSoft` | `(id, distance)` | — (world units, clamped ≥ 0; 0 = off) |
| `Particles.SetLit` | `(id, lit)` | — |

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

**Render modes.** `mode` is one of (case-insensitive):

- `"billboard"` (default) — a quad facing the camera, spun by the particle's rotation.
- `"stretched"` — a quad stretched along the particle's velocity: length =
  `size × length_scale + speed × speed_scale` (`SetStretch`). Sparks, tracers,
  shell-ejection streaks. A particle at rest draws as a billboard.
- `"horizontal"` — flat on the ground plane (XZ), spun about Y. Splashes, rings.
- `"vertical"` — faces the camera but stays upright. Fire columns, standing smoke.
- `"mesh"` — each particle is the mesh set by `SetMesh` (a primitive name such as
  `"Box"`, or a model path), drawn with the named scene material exactly like an
  entity carrying it (lit, receives shadows, fogged; no shadow cast). Size scales
  the mesh; rotation tumbles it about a random per-particle axis. The emitter's
  colour and gradient do not apply — the material is the look. Casings, debris,
  glass (a Transparent material sorts with the glass).

**Flipbooks.** `SetFlipbook(id, columns, rows, opts)` treats the texture as a
`columns × rows` sprite sheet read left-to-right, top-to-bottom; each particle
plays it over its life. `opts`: `cycles` (plays per life, default 1) and
`random_start` (start each particle on a random frame, default false). `1, 1`
is a plain sprite.

**Soft and lit.** `SetSoft(id, d)` fades each sprite out over the last `d` world
units before the surface behind it, so smoke never cuts a hard line into a wall.
`SetLit(id, true)` shades sprites by the scene: ambient (the light-probe field
when one covers the emitter) plus the directional, point and spot lights. Unlit
sprites show their own colour — right for fire and muzzle flashes, wrong for smoke.

Alpha-blended particles are drawn back to front within each emitter, and emitters
back to front among themselves.

```lua
Particles.SetRenderMode(sparks, "stretched")
Particles.SetStretch(sparks, 1, 0.05)
Particles.SetFlipbook(smoke, 4, 4, { random_start = true })
Particles.SetSoft(smoke, 0.5)
Particles.SetLit(smoke, true)
Particles.SetRenderMode(casings, "mesh")
Particles.SetMesh(casings, "assets/models/casing.glb", "Brass")
```

Over-life curves (size, colour gradient, drag, rotation speed) are authored in the
inspector or the scene file, not scripted.
