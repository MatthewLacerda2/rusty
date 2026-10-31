## `Particles`

Drive an entity's particle emitter (`ParticleEmitterComponent`). `Emit`/`Burst`
spawn at the entity's world position (a child's included) through the emitter's
own seeded path, so scripted emission stays
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
| `Particles.SetShape` | `(id, kind, opts?)` | `bool` (false for an unknown kind; radii and a box's size clamped ≥ 0, a cone's angle to `[0, 90]`) |
| `Particles.SetDirection` | `(id, x, y, z)` | — (the shape's axis / launch direction, in the emitter's space) |
| `Particles.SetLifetime` | `(id, min, max?)` | — (seconds, clamped ≥ 0) |
| `Particles.SetSpeed` | `(id, min, max?)` | — (a range with a NaN or infinite end is ignored, as for every range here) |
| `Particles.SetSize` | `(id, min, max?)` | — (start size, clamped ≥ 0) |
| `Particles.SetColor` | `(id, r, g, b, a?)` | — (start tint; `a` defaults to 1; a NaN or infinite channel leaves the tint as it was) |
| `Particles.SetSubEmitter` | `(id, trigger, target?)` | `bool` (false for an unknown trigger) |
| `Particles.SetRenderMode` | `(id, mode)` | `bool` (false for an unknown mode) |
| `Particles.GetRenderMode` | `(id)` | mode name, or `nil` without an emitter |
| `Particles.SetStretch` | `(id, length_scale, speed_scale?)` | — (each clamped ≥ 0) |
| `Particles.SetMesh` | `(id, mesh?, material?)` | — (`nil` clears) |
| `Particles.SetFlipbook` | `(id, columns, rows, opts?)` | — |
| `Particles.SetSoft` | `(id, distance)` | — (world units, clamped ≥ 0; 0 = off) |
| `Particles.SetLit` | `(id, lit)` | — |
| `Particles.SetSimulationSpace` | `(id, space)` | — (`"World"` or `"Local"`, any case; an unknown name is an error) |
| `Particles.GetSimulationSpace` | `(id)` | space name, or `nil` without an emitter |

**Ranges.** `SetLifetime`/`SetSpeed`/`SetSize` take `min, max`: each particle
draws its start value uniformly between them from the emitter's seeded stream.
Omit `max` for a constant.

**Shapes.** `kind` is one of `Point`, `Sphere`, `Hemisphere`, `Box`, `Cone`,
`Circle` (case-insensitive). `opts` is a table: `radius` (default 1), `angle`
(cone half-angle in degrees, default 25), `x`/`y`/`z` (box size, default 1) and
`surface` (`true` emits from the surface / edge instead of the volume). Shapes are
oriented by the emitter's direction, in the emitter's own space (its rotation
turns it; an unrotated emitter's direction is a world direction), so aiming a
hemisphere of sparks on an unrotated emitter along an impact normal is one
`SetDirection` call:

```lua no-run
Particles.SetShape(sparks, "Hemisphere", { radius = 0.05 })
Particles.SetDirection(sparks, nx, ny, nz)
Particles.Burst(sparks)
```

**Simulation space.** Unity's *Simulation Space*. `"World"` (the default): a
particle, once born, stays where physics takes it, and the emitter moving leaves
it behind (smoke, debris, a trail of sparks). `"Local"`: live particles keep
their position, velocity and orientation relative to the emitter, so they move,
turn and scale with it and with any parent bone (a muzzle flash or jet parented
to a gun, an exhaust on a vehicle); a mesh particle turns with the emitter on
top of its own tumble. In **both** spaces the shape and `SetDirection` are in
the emitter's own space, as Unity's Shape module is: a world-space emitter on a
muzzle launches along the gun, then leaves its particles behind. The emitter's
frame follows Unity's default *Local* Scaling Mode: its world position and
rotation under its **own** scale, every parent's scale ignored, so a viewmodel
rig at a 0.01 root scale does not shrink its flash; the emitter's own scale
scales the shape, the launch speed and a local emitter's live particles.
Particle size is never scaled. An emitter scaled to zero keeps its local
particles where they were and puts them back when it is scaled up again.
Gravity always pulls in world space. A sub-emitter fired from a local particle
spawns at that particle's world position. Particles are drawn, lit, sorted and
collided at their resulting world position, so every render mode looks the same
in either space. Unity's third option, `Custom` (relative to another entity), is
not offered yet: the scene format has no field that references another entity
by value.

```lua no-run
Particles.SetSimulationSpace(muzzle_flash, "Local")
Particles.Burst(muzzle_flash) -- stays on the gun while the player turns
```

**Sub-emitters.** `trigger` is `"birth"`, `"death"` or `"collision"`; `target` is
another entity with an emitter (usually a child whose own emission is off,
`Particles.SetActive(target, false)`), or `nil` to clear. The target entity must be
active in the hierarchy: an inactive one never fires.
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

```lua with=sparks,smoke,casings
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
