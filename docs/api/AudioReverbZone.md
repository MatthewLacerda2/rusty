## `AudioReverbZone`

Read and tune an entity's `ReverbZoneComponent` (#469, `src/api/reverb_zone.rs`) —
Unity's `AudioReverbZone`. A zone is a **sphere** around the entity's world
position that sets the reverb of the world mix while the listener (the active
camera) is in it: full effect within `min_distance`, fading linearly to nothing at
`max_distance` (a fresh zone: 10 m and 15 m, the `Room` preset). Use one for every
space that should sound different — a concrete tunnel, a small room, a hall — and
overlap them through doorways: where zones overlap, the listener hears their
**weighted blend** (see [Reverb zones](Audio.md#reverb-zones) for the rule and
`Audio.GetReverbState` for the result).

A zone carries a **preset** and four **params**; the params are what plays.
Picking a preset writes its params; setting params makes the zone `Custom`
(Unity's `User`, which `SetPreset` also accepts).

| Preset | `decay_time` (s) | `pre_delay` (s) | `damping` | `wet` | Sounds like |
|---|---|---|---|---|---|
| `Off` | 1.0 | 0 | 0.5 | 0 | dry — a zone that cancels the reverb around it |
| `Room` | 0.4 | 0.005 | 0.5 | 0.35 | a small room: short and dense |
| `Hall` | 1.8 | 0.02 | 0.3 | 0.45 | a large hall: long and smooth |
| `Tunnel` | 2.8 | 0.015 | 0.1 | 0.55 | a concrete tunnel: long, bright tail |
| `Outdoor` | 1.0 | 0.04 | 0.7 | 0.08 | open air: almost dry |
| `Custom` | — | — | — | — | the params as set |

**Params.** `decay_time` — how long the tail rings, in seconds (time to fall 60 dB),
`[0.1, 20]`. `pre_delay` — the gap before the tail starts, in seconds, `[0, 0.3]`.
`damping` — how fast the tail loses its highs, `[0, 1]` (0 is bright). `wet` — how
loud the tail is, linear `[0, 1]`. Out-of-range values are clamped.

Getters return `nil` without an `AudioReverbZone`; setters are then an error naming
the entity. Add or remove one with `Scene.AddComponent(id, "AudioReverbZone")`
(alias `ReverbZone`) / `RemoveComponent`. `Debug.Snapshot` shows it as
`reverb_zone`.

| Function | Signature | Returns |
|---|---|---|
| `AudioReverbZone.GetMinDistance` | `(id)` | `number` — the full-effect radius in metres |
| `AudioReverbZone.SetMinDistance` | `(id, d)` | — floored at 0; `max_distance` grows to stay at or past it |
| `AudioReverbZone.GetMaxDistance` | `(id)` | `number` — the radius where the effect has faded out |
| `AudioReverbZone.SetMaxDistance` | `(id, d)` | — never under `min_distance` |
| `AudioReverbZone.GetPreset` | `(id)` | `string` — the preset name (`Custom` after a param edit) |
| `AudioReverbZone.SetPreset` | `(id, name)` | — writes the preset's params (`Custom` keeps the params); case-insensitive, an unknown name is an error listing the presets |
| `AudioReverbZone.GetPresets` | `()` | `{string}` — every preset name, in menu order |
| `AudioReverbZone.GetParams` | `(id)` | table — `decay_time`, `pre_delay`, `damping`, `wet` |
| `AudioReverbZone.SetParams` | `(id, { decay_time, pre_delay, damping, wet })` | — changes the fields the table names (clamped) and makes the zone `Custom`; an unknown key is an error |

```lua
local tunnel = Scene.CreateEntity("B Tunnels")
Transform.SetPosition(tunnel, 40, 1, -12)
Scene.AddComponent(tunnel, "AudioReverbZone")
AudioReverbZone.SetPreset(tunnel, "Tunnel")
AudioReverbZone.SetMinDistance(tunnel, 6)     -- full inside the tunnel's width
AudioReverbZone.SetMaxDistance(tunnel, 9)     -- fades out over the mouth
AudioReverbZone.SetParams(tunnel, { wet = 0.7 })  -- wetter than the preset: now Custom
```
