## `Light`

Read and tune an entity's `LightComponent` — colour, intensity, range, type and
whether it casts shadows, and its mode.
Every setter maps onto a field the renderer reads when it packs the lighting
uniform, so a change takes effect on the next frame. A light has no per-component
"active" flag: it is gated by its owning entity's `active` (a `Scene` concern), so
this surface exposes the light's own data and nothing more. Getters return a
neutral default (`(1,1,1)` colour, `0` scalars, `"None"` type) when the entity has
no light.

| Function | Signature | Returns |
|---|---|---|
| `Light.GetColor` | `(id)` | `r, g, b` (linear) |
| `Light.SetColor` | `(id, r, g, b)` | — |
| `Light.GetIntensity` / `SetIntensity` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetRange` / `SetRange` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetType` / `SetType` | `(id)` / `(id, name)` | `"Ambient"` / `"Directional"` / `"Point"` / `"Spotlight"` |
| `Light.GetCastShadows` / `SetCastShadows` | `(id)` / `(id, on)` | `boolean` (default `false`; point and spot lights only) |
| `Light.GetMode` / `SetMode` | `(id)` / `(id, name)` | `"Realtime"` / `"Mixed"` / `"Baked"` |

`SetType` and `SetMode` are case-insensitive; an unrecognized name is ignored (the
current value is kept).

**How many lights shade.** Point and spot lights are rendered with clustered
forward lighting (#434): there is no fixed slot count. Each camera shades up to
**256** point/spot lights in its view; past that, the ones farthest from the camera
are dropped and counted in `Debug.Stats().lights_dropped`. A light outside the view
costs nothing. A light's **range** is a hard cut-off and also how far it reaches into
the cluster grid, so a tight range is the cheap one. Up to **4** directional lights
shade at once; the last active one is the sun that casts the cascaded shadows. Only
the last active ambient light counts.

**Shadows.** The sun (slot 0 above) always casts its cascaded shadows. A point or
spot light casts only when `SetCastShadows(id, true)` (off by default, as in Unity:
each one costs a depth pass per tile). Shadowed point and spot lights share one
2048² **shadow atlas** (#468): a spotlight takes one tile, a point light six (one
per cube face). Each frame the atlas goes to the most important lights the camera
can see — screen coverage times intensity — at 512² down to 128² per tile, nearer
and brighter lights getting the bigger tiles. A shadowed light that finds no room
still shades, unshadowed, and is counted in `Debug.Stats().shadow_lights_dropped`;
`shadowed_lights`, `shadow_atlas_tiles` and `shadow_atlas_texels` show how full the
atlas is. Static casters (objects flagged static) are cached per tile, as the sun's
cascades cache them: a light that does not move redraws only the dynamic casters in
its tiles each frame (`shadow_atlas_cached` / `shadow_atlas_rebaked` count it). A
moving light, or one whose tile moves or changes size as the camera moves, re-bakes.
The atlas holds sixteen 512² tiles, so one point light at full size takes
six of them. Every caster shadows
into the atlas as it does into the cascades: cutout and dissolving materials clip
their shadow, skinned meshes cast in their animated pose. Lit particles are not
shadowed.

**Mode** is Unity's Light Mode and decides what the bakes (`Lighting.BakeLightmaps`
for static surfaces, the light-probe bake for dynamic objects) do with the light, and
whether it is drawn live:

| Mode | Static, lightmapped surfaces | Dynamic objects (probe-lit) | Realtime shadows |
|---|---|---|---|
| `Realtime` | live direct light only, no bounce | live direct light only, no bounce | yes, if it casts shadows |
| `Mixed` *(default)* | live direct light + baked bounce | live direct light + bounce from the probes | yes, if it casts shadows |
| `Baked` | direct light and bounce from the lightmap | direct light and bounce from the probes only | no |

- `Realtime` — direct light and shadows every frame; neither bake sees it, so it
  bounces nowhere (a muzzle flash, a flickering lamp).
- `Mixed` — Unity's *Baked Indirect*: realtime direct light and shadows, plus its
  bounce baked into the lightmaps and the light probes. No shadowmask. The light
  that should cast sharp shadows on actors.
- `Baked` — costs nothing at runtime: it is never drawn live, anywhere. Static
  surfaces get it from their lightmap; dynamic objects get its direct light and its
  bounce from the light probes, whose bake adds the light's own direct light per
  probe (shadowed by static geometry). So a dynamic object lights from it only once
  probes are baked, softly, and it casts no realtime shadow; a static surface without
  a lightmap (no second UV map) does not receive it at all.
