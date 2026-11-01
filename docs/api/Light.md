## `Light`

Read and tune an entity's `LightComponent` — colour, intensity, range, type and
whether it casts shadows, and its mode.
Every setter but `SetAreaSize` maps onto a field the renderer reads when it packs
the lighting uniform, so a change takes effect on the next frame; an Area light is
baked only, so its size (and any change to an Area light) shows after the next
lightmap and probe bake. A light has no per-component
"active" flag: it is gated by whether its owning entity is active in the hierarchy (`Scene.IsActiveInHierarchy`), so
this surface exposes the light's own data and nothing more. Getters return a
neutral default (`(1,1,1)` colour, `0` scalars, `"None"` type and mode) when the
entity has no light. A spotlight's cone half-angles have no verb here: write
`inner_cone` / `outer_cone` with `Component.Set(id, "Light", {...})`
([`Component`](Component.md)). `SetType(id, "Spot")` on a light whose cones are both
`0` gives it 30° / 45°.

| Function | Signature | Returns |
|---|---|---|
| `Light.GetColor` | `(id)` | `r, g, b` (linear) |
| `Light.SetColor` | `(id, r, g, b)` | — (each channel floored at 0, HDR above 1 allowed; a NaN or infinite channel makes it black) |
| `Light.GetIntensity` / `SetIntensity` | `(id)` / `(id, value)` | `number` (clamped ≥ 0; NaN or `math.huge` reads as 0) |
| `Light.GetRange` / `SetRange` | `(id)` / `(id, value)` | `number` (clamped ≥ 0; NaN or `math.huge` reads as 0) |
| `Light.GetType` / `SetType` | `(id)` / `(id, name)` | `"Ambient"` / `"Directional"` / `"Point"` / `"Spot"` / `"Area"` (Unity's names) |
| `Light.GetCastShadows` / `SetCastShadows` | `(id)` / `(id, on)` | `boolean` (default `false`; point and spot lights, and an Area light's stand-in in a bake capture: the lightmap and probe bake shadow an Area light always) |
| `Light.GetMode` / `SetMode` | `(id)` / `(id, name)` | `"Realtime"` / `"Mixed"` / `"Baked"` (an Area light reads `"Baked"` and refuses all but `"Baked"`, with a console warning) |
| `Light.GetAreaSize` / `SetAreaSize` | `(id)` / `(id, width, height)` | `width, height` in world units (default `1, 1`; each side floored at `0.01`; a NaN or `math.huge` side refuses the whole size, keeping the current one; `0, 0` with no light) |

`SetType` and `SetMode` are case-insensitive; an unrecognized name is ignored (the
current value is kept).

**Area lights** (#1674) are Unity's *Area (Rectangle)* light, and like Unity's
built-in pipeline rusty bakes them only: an Area light acts as
`Baked` whatever its stored mode (`GetMode` and `Component.Get` read `"Baked"`, and
`SetMode` refuses the rest, warning in the console). The stored mode is kept, so a
Mixed point light switched to Area and back is Mixed again. The
emitter is a `width × height` rectangle (`SetAreaSize`) centred on the entity,
spanning its local X and Y and shining from its front, local −Z, only. Intensity is
per unit area: a 2 × 1 panel gives out twice what a 1 × 1 one of the same
intensity does, and seen from far away it acts as a spot light of `intensity ×
area` whose light fades as the cosine off the panel's facing. `range` cuts it off
from its centre, like a point light's. The lightmap bake integrates the whole
rectangle, so its shadows have soft edges that widen with the panel's size, and the
light-probe bake adds its direct light (a fixed 8 × 8 grid of shadow rays per
probe) for dynamic objects. It is never drawn live, so a surface without a lightmap
and an object before the probes are baked get nothing from it. Transform scale
does not resize it; `SetAreaSize` does. The editor draws its rectangle, its facing
and its range. A Disc shape and realtime area lights are not there yet.

**How many lights shade.** Point and spot lights are rendered with clustered
forward lighting (#434): there is no fixed slot count. Each camera shades up to
**64** point/spot lights in its view, and any one pixel at most **32** of them (the
view is cut into clusters, and each cluster lists at most 32; #834). Past either
budget the lights that **contribute least** are dropped — brightness (intensity ×
colour luminance) weighed by how close their range comes, so a bright lamp across
the room outranks a dim one beside the camera. Ties go to the earlier entity, so
the same scene always drops the same lights. Camera drops are counted in
`Debug.Stats().lights_dropped`, cluster drops in `cluster_lights_dropped` (non-zero
means some corner of the view is over budget). `Baked` lights never take part. A
light outside the view costs nothing. A light's **range** is a hard cut-off and also how far it reaches into
the cluster grid, so a tight range is the cheap one. Up to **4** directional lights
shade at once; the last active one is the sun that casts the cascaded shadows. Only
the last active ambient light counts: it replaces the scene's ambient, a `Gradient`
one included, with its single colour (`Lighting.md`, *Environment*).

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

### Cast Shadows

Each mesh says whether and how it casts, Unity's *Cast Shadows*
(`MeshRenderer.shadowCastingMode`): the Mesh component's `cast_shadows` field,
written with `Component.Set(id, "Mesh", { cast_shadows = "Off" })`
([`Component`](Component.md)), read with `Component.Get`, shown in the Inspector's
Mesh card and saved with the scene and its prefabs.

| `cast_shadows` | What it does |
|---|---|
| `"On"` *(default)* | casts from the faces its material draws (its Render Face, [`Material`](Material.md): the front faces by default) |
| `"Off"` | casts nothing: not in the cascades, the atlas, or their static bakes, and blocks no baked light |
| `"TwoSided"` | casts from both faces: a one-sided card or `Plane` shadows whichever way it faces the light |
| `"ShadowsOnly"` | casts as `"On"`, but no camera draws it: an invisible shadow stand-in |

Every shadow path reads it on its next frame, a static mesh's cached shadow bake
included. Whether a surface *receives* shadows is its material's switch,
`Material.SetReceiveShadows` ([`Material`](Material.md)).

The bakes (the lightmap and the light probes, [`Lighting`](Lighting.md)) follow one
rule, Unity's: *casting* decides whether a mesh blocks light, *being seen* decides
whether it bounces, emits and is lightmapped.

- `"Off"` never blocks light (a ray toward a light passes through it), but it is
  still seen: it bounces its colour, its emission still lights the scene, and it is
  lightmapped. An `Off` glowing sign still lights the wall beside it.
- `"ShadowsOnly"` blocks light, but nothing sees it: it bounces nothing, emits
  nothing, gets no lightmap and is left out of the probe captures.
- The editor picks and outlines by what is drawn: clicking a `"ShadowsOnly"` mesh in
  the viewport selects nothing, and selecting it in the hierarchy draws no outline.

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
