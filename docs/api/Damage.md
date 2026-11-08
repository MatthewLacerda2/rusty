## `Damage`

Tell an entity "you were hit" (#1893, `src/api/damage/`). Unity has no damage
system (projects `SendMessage("ApplyDamage")`), so this is **Unreal's**:
`ApplyDamage`, `ApplyPointDamage` and `ApplyRadialDamageWithFalloff`, delivered to
the target's [`OnTakeDamage`](index.md) callback. The engine stores **no health**:
what damage *does* (health, armour, a headshot multiplier, a death animation) is
the game's Lua, as Unreal leaves `TakeDamage` to the actor. The engine gives the
delivery, the hit information and the radial query.

| Function | Signature | Returns |
|---|---|---|
| `Damage.Apply` | `(target, amount [, opts])` — plain damage to `target` (`event.kind = "any"`). A `nil` target is no entity. | the damage taken; `0` when nothing received it |
| `Damage.ApplyPoint` | `(hit, amount, dx, dy, dz [, opts])` — a shot: `hit` is a [`Physics.RaycastAll` / `RaycastThrough`](Physics.md) entry, `dx, dy, dz` the shot's direction (`event.kind = "point"`). A `RaycastThrough` crossing's `enter.point`, `enter.normal` and `enter.distance` stand in for the missing top-level ones. An error when `hit` has no `id`. | the damage taken; `0` when nothing received it |
| `Damage.ApplyRadial` | `(x, y, z, amount, opts)` — a blast centred at `x, y, z` (`event.kind = "radial"`); see *Radial damage* below. `opts.outer` is required. | an array of `{ id, amount, distance }`, one per receiver, nearest first |

A harness scenario's `Damage.*` runs in the gameplay VM, so it reaches the game
scripts' handlers too (`docs/testing.md`, *What a scenario can call*).

`amount` and every number in `opts` must be finite (`0/0` is an error). In edit mode
(no physics world) `ApplyRadial` finds nothing and returns `{}`; `Apply` and
`ApplyPoint` still deliver.

**`opts`**, every field optional:

- `type` — the damage type, a free string the game picks (`"bullet"`,
  `"explosion"`, `"fire"`, `"melee"`); `"generic"` when absent. Unreal's
  `DamageType` class, kept as data.
- `instigator` — who caused it (the shooter), an entity id.
- `causer` — what did it (the gun, the grenade), an entity id. A radial blast
  ignores its causer and everything under it, as Unreal does: they are neither
  damaged nor pushed by it, and never shield anyone from it.
- `impulse` — N·s. `ApplyPoint` pushes the body the hit collider belongs to (its
  nearest `Rigidbody` up the hierarchy: a crate, a ragdolled bone) along the shot,
  at `hit.point` (a crossing's `enter.point`), with [`Physics.AddImpulseAtPosition`](Physics.md) itself; a
  kinematic or bodiless one doesn't move. `ApplyRadial` pushes every body in
  reach and clear of cover away from the origin (from its collider's bounds
  centre when the origin is on it), scaled by the falloff, whether or not it
  takes damage.

### Who receives it

The event **bubbles**: it goes to the nearest entity from the one named (or the
hit's `id`) **up** whose live script defines `OnTakeDamage` and that is active —
the rule the UI pointer callbacks follow. So a hit on a hitbox under `hand_r`
reaches the character's script, not the bare bone, without the game routing it
through the hit's `root` (the top of the hierarchy, which can be a level group).
Nothing up the hierarchy handling it means nothing receives it.

Every script on the receiver that defines `OnTakeDamage` is called, in script-slot
order, **synchronously** inside the `Damage.*` call (Unity's `SendMessage`,
Unreal's `TakeDamage`), through the live instances `Scene.GetScript` reads. A
handler may return a number, the damage it actually took (armour, a multiplier);
`nil` means it took what it was given. The next script's handler is given that
number, and the last one is what `Apply*` returns (Unreal's `ActualDamage`).

A handler may apply damage itself (a barrel that explodes when shot). Calls nest
up to **16** deep; the 17th raises `Damage.<verb>: damage nested deeper than 16
calls`, and that error unwinds every call under it — the console shows it once, as
the error of the script callback that started the chain. A handler's own error
propagates the same way.

### `OnTakeDamage(id, amount, event)`

`id` is the receiver, `amount` the damage it is given. `event` is a table:

| Field | Meaning |
|---|---|
| `kind` | `"any"`, `"point"` or `"radial"` (Unreal's three damage events, one callback) |
| `type` | the damage type (`opts.type`, `"generic"` by default) |
| `instigator` | `opts.instigator`, or `nil` |
| `causer` | `opts.causer`, or `nil` |
| `hit` | `point` only: a copy of the hit table passed in (`id`, `point`, `normal`, `distance`, `bone`, `bone_name`, `root`, …; for a `RaycastThrough` crossing also `thickness`, `enter`, `exit`, with `enter`'s `point`, `normal` and `distance` copied up) plus `direction = {x, y, z}`, the shot's unit direction |
| `origin` | `radial` only: the blast centre, `{x, y, z}` |
| `distance` | `radial` only: how far the receiver was from it (its nearest reached collider's closest point) |

```lua no-run
-- bot.lua: health is the game's
function Bot.OnTakeDamage(id, amount, event)
  if event.kind == "point" and event.hit.bone_name == "head" then amount = amount * 4 end
  Bot.health = Bot.health - amount
  return amount
end

-- the gun
local hits = Physics.RaycastAll(ox,oy,oz, dx,dy,dz, 100, SHOT, false)
if hits[1] then
  Damage.ApplyPoint(hits[1], 25, dx,dy,dz, { type = "bullet", instigator = self_id, impulse = 2 })
end
```

### Radial damage

`ApplyRadial` is Unreal's `ApplyRadialDamageWithFalloff`, for grenades and
barrels. Its `opts` take, beside the common fields:

- `outer` (**required**) — metres; nothing past it is touched.
- `inner` — full damage inside it (default `0`). `0 <= inner <= outer`.
- `min` — the damage at `outer` (default `0`).
- `falloff` — the exponent of the drop from `inner` to `outer` (default `1`, linear;
  must be `> 0`): `min + (amount - min) · (1 - t)^falloff`, `t` the fraction of the
  way from `inner` to `outer`.
- `layer_mask` — the layers it looks for (default every layer).
- `blocking_mask` — the layers that stop it (default every layer). Set it to your
  world geometry's layers so bodies don't shield each other.

How it finds its victims:

1. **Candidates.** Every solid (non-trigger) collider on `layer_mask` touching the
   sphere of radius `outer` ([`Physics.OverlapSphere`](Physics.md)), except the
   `causer`'s, grouped by who receives it (the bubbling rule above), so a
   character with fifteen hitboxes is damaged **once**. Colliders with no
   receiver group by their body, for the push.
2. **Cover**, per collider (Unreal's `ComponentIsDamageableFrom`). A collider is
   reached when a ray from the origin to its closest point
   ([`Physics.ClosestPoint`](Physics.md)), or to its bounds centre, meets nothing
   on `blocking_mask` before it. A group is reached when **any** of its colliders
   is: a bot crouched behind a crate with its head in view takes the blast. The
   group's own colliders and the `causer`'s never block. A grenade behind a wall
   does nothing to the room beyond. A blocker the origin lies on or inside (a
   grenade at a raycast's `hit.point` on a wall) doesn't stop a ray leaving it,
   nor any ray when the origin is buried in it; it still stops a ray from its
   surface into it, so a blast on a wall's face doesn't reach through the wall.
3. **Distance.** The nearest *reached* collider's closest point; the falloff and
   `event.distance` use it.
4. **Order.** Groups are processed nearest first, ties by entity id, so two runs
   over one scene give the same list. A receiver that an earlier handler's blast
   deactivated is skipped.

```lua no-run
function Barrel.OnTakeDamage(id, amount, event)
  -- its own blast leaves it alone; this is for a neighbouring barrel's
  if Barrel.exploded then return 0 end
  Barrel.exploded = true
  local x, y, z = Transform.GetPosition(id)
  Damage.ApplyRadial(x, y, z, 100, { inner = 1, outer = 6, type = "explosion",
    causer = id, impulse = 40, blocking_mask = 1 << Layers.NameToIndex("Default") })
  return amount
end
```
