## `Decals`

Stamp **surface decals** (bullet holes, scorch, blood splats) onto the surface a
shot hit. A decal is a projected *box*, not a flat sticker: it wraps whatever
geometry the box overlaps, and it changes that surface's **material before the
surface is lit** (#638) — its albedo, normal, metallic, roughness and occlusion.
So a decal is lit, shadowed and occluded exactly like the wall it lands on: blood
goes dark in a dark corridor, a wet patch catches the flashlight's highlight, a
bullet hole's rim catches the muzzle flash. Spawn it from the hit point and surface
normal the cast already returns, and pass the entity it hit as the `owner` so the
mark moves with it. The registry is a bounded FIFO (256): past the cap the oldest
decal fades out rather than vanishing.

```lua
local hit, id, dist, px, py, pz, nx, ny, nz = Physics.Raycast(ox,oy,oz, dx,dy,dz, self_id)
if hit then Decals.Spawn(px, py, pz, nx, ny, nz, { owner = id }) end
```

| Function | Signature | Returns |
|---|---|---|
| `Decals.Spawn` | `(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])` or `(x,y,z, nx,ny,nz, opts)` | the decal's id |
| `Decals.Remove` | `(id, [fade])` | `true` if a live decal had that id |
| `Decals.Count` | `()` | live decal count, including any still fading out |
| `Decals.Clear` | `()` | — (drops every live decal) |

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
keys `owner`, `lifetime` and `fade` (below). An unknown key, a material that does
not exist, or an `owner` entity that does not exist is an error and stamps nothing.

```lua
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
- **`Decals.Remove(id, [fade])`** removes one decal now, or fades it out over
  `fade` seconds. Ids are never reused, so a stale id simply returns `false`.
- **The cap.** At most 256 decals are whole at once. Each new decal past that
  starts the oldest whole one fading out over 0.5 s; a fading decal no longer
  counts against the cap. Only if more than 64 are fading at once (above 128
  stamps a second, sustained) is the oldest dropped outright, so the frame never
  holds more than 320.

```lua
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
