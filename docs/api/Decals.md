## `Decals`

Stamp **surface decals** (bullet holes, scorch, blood splats) onto the surface a
shot hit. A decal is a projected *box*, not a flat sticker: it wraps whatever
geometry the box overlaps, and it changes that surface's **material before the
surface is lit** (#638) — its albedo, normal, metallic, roughness and occlusion.
So a decal is lit, shadowed and occluded exactly like the wall it lands on: blood
goes dark in a dark corridor, a wet patch catches the flashlight's highlight, a
bullet hole's rim catches the muzzle flash. Spawn it from the hit point and surface
normal the cast already returns, and pass the entity it hit as the `owner` so the
mark moves with it. Decals are budgeted per owner (256 each), in the world (1024)
and in total (4096): past a budget the oldest decal of that budget fades out
rather than vanishing.

```lua no-run
local hit, id, dist, px, py, pz, nx, ny, nz = Physics.Raycast(ox,oy,oz, dx,dy,dz, self_id)
if hit then Decals.Spawn(px, py, pz, nx, ny, nz, { owner = id }) end
```

| Function | Signature | Returns |
|---|---|---|
| `Decals.Spawn` | `(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])` or `(x,y,z, nx,ny,nz, opts)` | the decal's id |
| `Decals.Remove` | `(id, [fade])` | `true` if a live decal had that id |
| `Decals.Count` | `()` | live decal count, including any still fading out |
| `Decals.Clear` | `([{ persistent = false }])` | — (drops every live decal; with `persistent = false`, all but the persistent ones; `persistent` must be a boolean) |
| `Decals.SetLimits` | `({ per_owner =, world =, total = })`, any subset | — (lowering one fades the oldest excess) |
| `Decals.GetLimits` | `()` | `{ per_owner, world, total }`, the budgets in force (`project.rusty` spells them `max_per_owner`, `max_world`, `max_total`) |

**Positional form.** `size` (default `0.5`) is the stamp's width/height in world
units; `texture` is the albedo path (default: none, a solid square of the tint);
`rotation_deg` spins the stamp around its projection axis; `r,g,b,a` tints it
(default opaque white; alpha scales its coverage). The box reaches `max(size, 0.5)`
through the surface, centred on the hit. A positional decal changes the **albedo
only**.

**`opts` form.** A table in place of the trailing arguments, every key optional:
`size`, `depth` (how far the box reaches, centred on the hit), `rotation`
(degrees), `color` (`{r, g, b, a}`), `texture`, and **`material`** — the name of a
library material (`Material.DefineAsset`) the decal stamps — plus the lifecycle
keys `owner`, `lifetime`, `fade` and `persist` (below). An unknown key, a material that does
not exist, an `owner` entity that does not exist, or a `persist` that is not `true`
or `false` is an error and stamps nothing.

```lua no-run
Material.DefineAsset("blood", {
  base_color = {0.35, 0.0, 0.0}, base_color_map = "decals/blood.png",
  roughness = 0.1,                 -- wet
  decal = { metallic = 0 },        -- leave the wall's metalness alone
})
Decals.Spawn(px, py, pz, nx, ny, nz, { material = "blood", size = 1.2, rotation = math.random(0, 359) })
```

### Sticking to what was hit, and ageing out

- **`owner`** (an entity id): the decal sticks to that entity. Its box is kept in
  the entity's space, so a bullet hole on a swinging door, a sliding crate or a
  moving car stays on it. While the owner is inactive its decals are hidden (they
  come back with it); when it is destroyed they are dropped. Any entity works,
  including a skinned character's **bone** (bones are GameObjects): the mark
  follows that bone rigidly, though it does not bend with the skin around a joint.
  Without an owner a decal stays where it was stamped in world space.
- **`lifetime`** (seconds): the decal is removed once it is this old. **`fade`**
  (seconds, default `0`): it fades out over the last `fade` seconds of its
  lifetime, so blood dries away and scorch cools instead of popping. Age is game
  time on the fixed tick: it slows with `Time.SetTimeScale` and stops at scale `0`, and a replay ages
  every decal identically. Decals age only in Play.
- **`persist = true`** keeps the decal across `Scene.Load` (#1518), for a trail of
  destruction that outlives a level reset (`Scene.Load(Scene.GetActivePath())`).
  Without it a decal goes away with its scene. A persistent decal keeps its id,
  its age and what is left of its `lifetime` and `fade`, and its place in the blend
  order: everything spawned after the load lands over it. Where it lands:
  - **an owner that survives** (`Scene.DontDestroyOnLoad`): it stays on it;
  - **an owner that does not**: it is re-attached, in the same place on the owner,
    to the incoming entity at the owner's **place in the hierarchy**: the same
    names from the root down, and among siblings of the same name the same one in
    order (the third `Crate` under `Arena` comes back on the third), so a mark on a
    crate comes back on that crate at its reset position. A bone owner works the
    same way, by its bone's place. When the new scene has no entity there (another
    level, a spawned enemy) the decal is dropped;
  - **no owner**: it stays where it is in world space.

  Carried decals count against their budgets again, oldest first, a re-attached one
  against its new owner's. Persistence is for `Scene.Load` within a run only: it is
  not saved to disk, and Stop puts back the decals the scene had in edit mode,
  dropping every one Play stamped, as Unity reverts play-mode changes.
  `Decals.Clear()` drops the persistent ones too (game over, back to the menu);
  `Decals.Clear({ persistent = false })` drops only the rest.
- **`Decals.Remove(id, [fade])`** removes one decal now, or fades it out over
  `fade` seconds. Ids are never reused, so a stale id simply returns `false`.
- **The budgets.** Every decal counts against one bucket: its **owner** (each
  entity has its own, and a bone is its own owner), or the **world** for a decal
  stamped without one. So shooting the floor for a minute never erases the marks
  on a crate across the room.

  | Budget | Default | `project.rusty` key | Counts |
  |---|---|---|---|
  | per owner | 256 | `max_per_owner` | whole decals owned by one entity |
  | world | 1024 | `max_world` | whole decals with no owner |
  | total | 4096 | `max_total` | every whole decal in the scene (a performance ceiling) |

  A new decal past its bucket's budget starts the **oldest decal of that bucket**
  fading out over 0.5 s; past the total, the oldest whole decal in the scene fades.
  A fading decal no longer counts against any budget. Only if more than 64 decals
  that spawns evicted are fading at once (above 128 evictions a second, sustained)
  is the oldest of them dropped outright, so a stream of spawns never holds the
  frame above the total plus 64. A fade a script asks for (`Decals.Remove(id,
  fade)`) and one a lowered limit starts are never cut short: every decal they
  touch fades out in full, however many. Splitting into buckets never changes the
  blend order: newest still lands on top.
- **Setting the budgets.** A game sets its own in `project.rusty`:
  `"decals": { "max_per_owner": 256, "max_world": 1024, "max_total": 4096 }` (any
  key left out keeps its default). Edit mode runs on them, entering Play applies
  them, and an edit to the file is picked up by the next Play. An unknown key or a
  value that is not a whole number is skipped with a console warning, and so is a
  file that can't be read or parsed, which keeps the budgets of the last good read.
  A warning is given once per edit of the file, not again on every Play and Stop.
  `Decals.SetLimits{ world = 512 }` changes them until Stop, which puts the
  project's back, as Unity reverts play-mode changes (a key left out keeps its
  value, an unknown key is an error), and lowering one fades the oldest excess out
  at once. `Decals.GetLimits()` reads what is in force; note that it and
  `SetLimits` spell the keys `per_owner`, `world`, `total`, while the file spells
  them `max_per_owner`, `max_world`, `max_total`.

```lua no-run
-- blood that dries off a moving enemy's arm after 20 s
local hit, id, _, px, py, pz, nx, ny, nz = Physics.Raycast(ox,oy,oz, dx,dy,dz, self_id)
if hit then
  local splat = Decals.Spawn(px, py, pz, nx, ny, nz,
    { material = "blood", owner = id, lifetime = 20, fade = 5 })
end
```

### What a decal material changes

A decal material is an ordinary library material (Unity's way: a decal projector
points at a material). The decal reads its `base_color` × `base_color_map` (albedo,
the map's alpha is the decal's shape), `alpha`, `normal_map`, `metallic` ×
`metallic_map` and `roughness` × `roughness_map`, and its **`decal`** block says how
strongly each replaces the receiving surface's own, scaled by the decal's coverage:

| `decal` key | Default | Meaning |
|---|---|---|
| `albedo` | `1` | weight of the decal's albedo |
| `normal` | `1` | weight of the decal's normal map (no map, no bend) |
| `metallic` | `1` | weight of the decal's metallic |
| `roughness` | `1` | weight of the decal's roughness |
| `occlusion` | `1` | ambient occlusion the decal adds: `1` none, `0.5` halves the indirect light |
| `angle_fade` | `60` | degrees: whole on a surface turned up to this far from facing the projector, gone 15° past it |

Weights clamp to `[0, 1]`, `angle_fade` to `[0, 90]`. A decal that changes only
`roughness` is a wet patch; one that changes only `normal` is a dent. The tint
(`color`) multiplies the material's colour and alpha.

### Where decals land

- **Angle fade.** A surface running along the projection axis (the floor next to a
  wall hit, the side of a door frame) fades the decal out instead of streaking it.
- **Opting out.** A material with `receive_decals = false` never takes decals — set
  it on characters and props that walk through bullet holes. Transparent materials,
  unlit surfaces and mesh particles never receive decals.
- **The viewmodel.** A `DepthOnly` camera stacked over another (the FPS viewmodel,
  an overlay) shows no world decals, whatever its materials say.
- **Order.** Decals blend in spawn order (FIFO): a newer decal lands over an older
  one where they overlap.
- **Fading** scales the decal's coverage, so every channel it changes (albedo,
  normal, metallic, roughness, occlusion) fades together.

### Limits and cost

Decals are binned into the same clusters as the point and spot lights; a surface
pays only for the decals whose box reaches its cluster, and decals draw nothing of
their own. Their maps live in one 32-layer texture array at 512² per map (any
size is resized); past 32 distinct maps in one frame, the extras draw without
their texture (`decal_maps_dropped` in `Debug.FrameStats`). `decals_visible` and
`decal_cluster_refs` count the decals and the (cluster, decal) pairs each frame.
