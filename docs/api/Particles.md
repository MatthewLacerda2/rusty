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
| `Particles.SetTexture` | `(id, path?)` | — (a project-relative image; `nil` or `""` goes back to the default soft disc) |
| `Particles.GetTexture` | `(id)` | the sprite's image path, or `nil` (the default sprite, or no emitter) |
| `Particles.SetFlipbook` | `(id, columns, rows, opts?)` | — |
| `Particles.SetSoft` | `(id, distance)` | — (world units, clamped ≥ 0; 0 = off) |
| `Particles.SetLit` | `(id, lit)` | — |
| `Particles.SetReactive` | `(id, name)` | — (`"Auto"`, `"Off"` or `"Full"`, any case; an unknown name is an error): the sprites' reactivity under TAA (`Graphics.md` → *TAA*, #1845); `Auto`, the default, from their alpha (Alpha) or what they add (Additive). Mesh particles take their material's |
| `Particles.GetReactive` | `(id)` | reactivity name, or `nil` without an emitter |
| `Particles.SetSimulationSpace` | `(id, space)` | — (`"World"` or `"Local"`, any case; an unknown name is an error) |
| `Particles.GetSimulationSpace` | `(id)` | space name, or `nil` without an emitter |
| `Particles.GetParticles` | `(id)` | an array of particle tables, oldest first (*Custom logic*); empty without an emitter |
| `Particles.SetParticles` | `(id, list)` | the number of particles now live (0 without an emitter); an entry that is not a table, or a field of the wrong type, is an error that changes nothing |
| `Particles.ForEach` | `(id, fn)` | the number of particles now live (`fn` not a function is an error): `fn(p)` runs once per live particle, oldest first, on **one reused table** whose edits are written back; returning `false` kills the particle |
| `Particles.GetArrays` | `(id, fields)` | a table holding one array of numbers per name in `fields` (`{ "x", "vy" }`), index-aligned with the live particles, oldest first; empty arrays without an emitter; an unknown name is an error |
| `Particles.SetArrays` | `(id, arrays)` | the number of particles now live (0 without an emitter): writes back the arrays a `GetArrays` table still holds; a stale, short or non-number array is an error that changes nothing |

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

**Texture.** `SetTexture(id, path)` sets the sprite image (the inspector's
*Texture* and `Component.Set(id, "Particles", { texture = … })` write the same
field). An emitter with no texture draws Unity's default particle: a soft white
disc whose alpha falls to 0 at its rim, tinted by the emitter's colour. A path
that fails to load draws the renderer's missing-texture checker instead, so a
typo shows. Mesh particles ignore it (the material is the look).

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
Particles.SetTexture(smoke, "assets/textures/smoke_sheet.png")
Particles.SetFlipbook(smoke, 4, 4, { random_start = true })
Particles.SetSoft(smoke, 0.5)
Particles.SetLit(smoke, true)
Particles.SetRenderMode(casings, "mesh")
Particles.SetMesh(casings, "assets/models/casing.glb", "Brass")
```

Over-life curves (size, colour gradient, drag, rotation speed) are authored in the
inspector or the scene file, not scripted.

**Custom logic.** A rule the emitter's modules cannot express (sparks pulled
toward a point, embers swirling, colour by speed, a custom kill rule) is plain Lua
in a script's `Update` or `LateUpdate`, written one of three ways. A particle is
flat and in world space, for a `World` and a `Local` emitter alike; each field is
one number:

- `x`, `y`, `z` — position (units); `vx`, `vy`, `vz` — velocity (units/second);
- `age`, `lifetime` — seconds; the particle dies when `age >= lifetime`;
- `size` — its start size (the size curve still scales it over life);
- `rotation` — degrees;
- `r`, `g`, `b`, `a` — its start tint (the colour gradient still scales it);
- `custom1` to `custom4` — four numbers, zero at birth, that the engine never
  reads: per-particle state a rule keeps (a phase, a target, a "has bounced"
  flag), Unity's *Custom Data*. They stay with their particle as older ones die
  and indices shift.

**Whole arrays, for a rule that runs every tick.** `GetArrays(id, fields)` reads
only the fields named, as one plain Lua array each, and `SetArrays(id, arrays)`
writes back every array the table still holds, index for index: a rule is then
one loop over arrays, with no call, table or string per particle (Unity's
job-system access, `ParticleSystemJobData`). Set a field's array to `nil` before
writing to leave that field alone. An entry left a NaN or an infinity keeps its
value; one set to `age >= lifetime` kills its particle (no `death` sub-emitter
fires, as below). The arrays can neither add particles nor remove one otherwise,
so every hidden field stays with its particle. They are tied to the read: once
the live particles are no longer the ones read (a tick aged them; an emit, a
kill or another write added, removed or aged one), `SetArrays` refuses them, as
it refuses an array of the wrong length or holding a non-number, and changes
nothing. Read and
write in the same function.

```lua
-- sparks pulled toward a point 2 m up
local FIELDS = { "x", "y", "z", "vx", "vy", "vz" }
return { Update = function(id, dt)
  local a = Particles.GetArrays(id, FIELDS)
  local x, y, z, vx, vy, vz = a.x, a.y, a.z, a.vx, a.vy, a.vz
  for i = 1, #x do
    vx[i] = vx[i] + (0 - x[i]) * 4 * dt
    vy[i] = vy[i] + (2 - y[i]) * 4 * dt
    vz[i] = vz[i] + (0 - z[i]) * 4 * dt
  end
  a.x, a.y, a.z = nil, nil, nil -- read, not written
  Particles.SetArrays(id, a)
end }
```

**`GetParticles` and `SetParticles`** are Unity's pair, one table per particle,
for a rule that adds, removes or reorders particles. `SetParticles` replaces the
live particles with `list`. Each table `GetParticles` handed out stays tied to
the particle it was read from, wherever the script moves it in the list, so
removing an entry (`table.remove`, or copying the survivors into a new list) and
reordering are safe. A field an entry leaves out, or gives a NaN or infinite
number, keeps its value; a field that is neither a number nor `nil` is an error
naming the entry and the field, and nothing is written. A shorter list kills the
rest, and an entry with `age >= lifetime` is dropped; neither fires the `death`
sub-emitter, since the script removed the particle and the engine never saw it
die. A table the script built itself adds a particle, up to `max_particles` (the
rest are dropped; the return value says how many are live): it starts at the
emitter, at rest, aged 0, with the lifetime, size and colour ranges' minimums and
its own fields written over them. What a script cannot set (a mesh particle's
tumble axis, its flipbook start frame, the turn a `Local` emitter gave it) stays
with its particle. A table is tied to its particle only until the emitter's next
read, write or tick: one from an older `GetParticles`, from before a
`SetParticles`, `ForEach` or `SetArrays` on that emitter, or read in `Update` and
written in `LateUpdate` is the script's own data, and adds a particle with its
fields and those defaults. Read and write in the same function.

**`ForEach(id, fn)`** is a per-particle rule without a table per particle: `fn`
gets one table, refilled for each particle, and only the fields it changed are
written back, as `SetParticles` would; `return false` kills the particle (no
`death` sub-emitter either). An error in `fn`, or a field of the wrong type left
in its table, changes nothing. Keep no reference to the table: once `ForEach`
returns (or fails), any read or write of it is an error.

Every way is deterministic like any script: `Update` and `LateUpdate` run inside
the fixed tick. `Update` runs before the tick integrates the particles, so it
reads last tick's state and the tick moves what it writes; `LateUpdate` runs
after, so it reads this tick's and the next tick moves what it writes (the frame
draws it as written). A `Local` emitter is carried to where it stands before any
read or write, even while it is inactive, so a script that moved it reads and
writes where its particles draw. Outside Play the calls read and write whatever
is live, as `GetCount` and `Emit` do. Neither `fn` nor the code between a read
and its write should emit into the same emitter: `SetParticles` and `ForEach`
replace what is live with what was read, and `SetArrays` refuses the write.

**Cost.** Measured on the committed bench (`tests/particles/live/bench.rs`), a
round trip over 1,000 particles in a release build: the arrays cost a few
hundredths of a millisecond per field touched (about 0.1 ms to change one
velocity component, about 0.3 ms for the attractor above); `ForEach` costs
four to ten times that for the same rule (about 0.7 ms for one write, 1 ms for the
attractor), since every particle is a Lua call and a refilled table; the pair
costs about twice `ForEach` (about 1.8 ms), since every particle is a new table.
At 60 ticks a second a frame's whole budget is 16 ms, so a rule over thousands
of particles every tick belongs in the arrays.
