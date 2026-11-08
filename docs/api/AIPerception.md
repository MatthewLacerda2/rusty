## `AIPerception`

What an agent **knows**: Unreal's AI Perception (`UAIPerceptionComponent` with the
*Sight*, *Hearing* and *Damage* senses, #1900). Unity has no perception system, so this
follows Unreal. A bot that reads `Transform.GetPosition(player)` knows where the player
is through walls; a bot with an `AIPerception` knows only what it saw, heard or was hit
by, and where that was. Feed what it knows to an [`EnvironmentQuery`](EnvironmentQuery.md)
and a state machine.

Add one with `Scene.AddComponent(id, "AIPerception")` (alias `Perception`). Its settings
save with the scene; what it knows is runtime state, empty on load and on Play.
`Debug.Snapshot` reports it as `ai_perception`, with the known and in-sight target ids.

### The senses

- **Sight**, stepped every fixed tick after physics and animation. A **sight target** is
  an active, non-static entity on the perceiver's `sight_mask` with a body to see (a
  `Collider` or a `CharacterController`), not the perceiver or under it, and not under
  another target (a character's hitboxes are the character). Static entities are the
  level and are never targets. A target is **seen** when it is within `sight_range`
  (once seen, `lose_sight_range`), inside the view cone (`sight_angle` degrees either
  side of the entity's forward, +Z; 180 sees all round), and one ray from the eye
  (`eye_height` above the origin) to the target's bounds centre meets no collider on
  `blocking_mask` (the perceiver's and the target's own colliders, and their
  descendants', never block; nor do triggers).
  Rays are **budgeted**: 64 a tick across every perceiver (Unreal's
  `MaxTracesPerTick`), served round-robin over the (perceiver, target) pairs in id
  order, from where the last tick stopped. A pair not traced this tick keeps its last
  answer, so 30 bots on a Dust2-sized map cost a bounded amount, and each pair is traced
  at least every `ceil(pairs / 64)` ticks. Out of range or out of the cone costs no ray.
- **Hearing**: `ReportNoise(x, y, z, loudness, opts)`. Every active perceiver within
  `hearing_range × loudness` of the point hears it. **Walls do not block hearing**, as in
  Unreal. The engine never reports a noise itself: playing a sound is not a noise,
  because the game decides what an enemy reacts to. A gun script calls `ReportNoise`
  beside its `Sfx` call. The instigator does not hear its own noise.
- **Damage**: every [`Damage`](Damage.md) `Apply` / `ApplyPoint` / `ApplyRadial` hit
  is sensed by the nearest active entity from the one struck upward that has an
  `AIPerception` (a hitbox under a bone is the character's), whether or not a script
  receives the damage. It is placed at the instigator's position when the instigator is
  alive, else at the hit point (`ApplyPoint`), the blast origin (`ApplyRadial`) or the
  struck entity (`Apply`); `amount` is the damage dealt, before handlers, and `tag` the
  damage type.

### What it knows

One **known target** per entity, as a table: `{ id, sense, x, y, z, age, sensed, tag,
amount }`. `sense` is `"sight"`, `"hearing"` or `"damage"`, the sense that placed it;
`x, y, z` is the **last-known position** (where the target stood when seen, the
noise's point, or where the instigator stood at the hit); `age` is seconds since it was
last sensed; `sensed` is true while it is sensed this tick (in view now, or heard or hit
this tick); `tag` is the noise's tag or the damage type; `amount` the damage of a hit.
While a target is in view, sight keeps its entry: a noise it makes is still reported
(the update fires) but does not move its last-known position.

An entry not sensed ages on the fixed tick and is **forgotten** once its age passes its
sense's max age (`sight_max_age`, `hearing_max_age`, `damage_max_age`; `0` keeps it
forever). A noise or hit with no instigator is sensed (its update fires with a `nil`
target) but stored under nobody: a known target is an entity.

### Updates

`OnPerceptionUpdated(id, target, info)` (Unreal's `OnTargetPerceptionUpdated`), on the
perceiver's scripts, with `info` the known-target table above, fires when a target is
**gained** (first sensed, or seen again after not being), when it is **lost** from
sight (`info.sensed == false`; it stays known at its last-known position), when it is
**forgotten** (`info.forgotten == true`; it is gone), and for **every** noise and hit,
so a script reacts to each gunshot. Updates are dispatched once a tick, after the
perception step and before `LateUpdate`, perceivers in id order. A noise reported in
`LateUpdate` is dispatched the next tick.

```lua with=bot,player
local Bot = {}
function Bot.OnPerceptionUpdated(id, target, info)
  if target == player and info.sense == "sight" and not info.sensed then
    -- Lost the player: go where it was last seen.
    NavMeshAgent.SetTarget(id, info.x, info.y, info.z)
  end
end
```

| Function | Signature | Returns |
|---|---|---|
| `AIPerception.GetKnown` | `(id)` | an array of known-target tables, nearest the perceiver first, ties by id (empty without the component) |
| `AIPerception.Get` | `(id, target)` | that target's known-target table, or `nil` when it isn't known |
| `AIPerception.CanSee` | `(id, target)` | `true` when the target is seen at the last sight step |
| `AIPerception.ReportNoise` | `(x, y, z, loudness [, opts])` | the number of perceivers that heard it. `opts.instigator` is who made it, `opts.tag` a free string (`"gunshot"`, `"footstep"`). A non-finite point or loudness, or a negative loudness, is an error |
| `AIPerception.Forget` | `(id [, target])` | — drops one known target, or all; no update fires, and a target still in view is seen again on the next step |
| `AIPerception.GetSightRange` / `SetSightRange` | `(id)` / `(id, metres)` | where sight starts (default 20); the lose-sight range is raised to it |
| `AIPerception.GetLoseSightRange` / `SetLoseSightRange` | `(id)` / `(id, metres)` | how far a seen target is kept (default 25), never below the sight range |
| `AIPerception.GetSightAngle` / `SetSightAngle` | `(id)` / `(id, degrees)` | the view cone's half-angle (default 60), clamped to 0–180 |
| `AIPerception.GetEyeHeight` / `SetEyeHeight` | `(id)` / `(id, metres)` | where the sight ray starts above the origin (default 1.6) |
| `AIPerception.GetSightMask` / `SetSightMask` | `(id)` / `(id, mask)` | the layers a target may be on (default all) |
| `AIPerception.GetBlockingMask` / `SetBlockingMask` | `(id)` / `(id, mask)` | the layers that block sight (default `1`, the `Default` layer) |
| `AIPerception.GetHearingRange` / `SetHearingRange` | `(id)` / `(id, metres)` | how far a noise of loudness 1 carries (default 30) |
| `AIPerception.GetSightMaxAge` / `SetSightMaxAge` | `(id)` / `(id, seconds)` | how long a target is remembered out of sight (default 5; 0 = forever) |
| `AIPerception.GetHearingMaxAge` / `SetHearingMaxAge` | `(id)` / `(id, seconds)` | how long a noise is remembered (default 3; 0 = forever) |
| `AIPerception.GetDamageMaxAge` / `SetDamageMaxAge` | `(id)` / `(id, seconds)` | how long a hit is remembered (default 5; 0 = forever) |

Distances and ages are floored at 0; a non-finite value is ignored (the setting keeps
its value). Getters return `0` without the component (or for a `nil` or dead id);
setters are then no-ops, and a setter (or `Forget`) given a `nil` id is an error.
