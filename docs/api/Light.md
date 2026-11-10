## `Light`

Read and tune an entity's `LightComponent` — colour, intensity, range, type and
whether it casts shadows (and how far from the camera), and its mode.
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
| `Light.GetShadowFadeDistance` / `SetShadowFadeDistance` | `(id)` / `(id, metres)` | `number`: how far from the camera a point or spot light's shadow reaches (default `0`, no fade; clamped ≥ 0; NaN or `math.huge` reads as 0) |
| `Light.GetMode` / `SetMode` | `(id)` / `(id, name)` | `"Realtime"` / `"Mixed"` / `"Baked"` (an Area light reads `"Baked"` and refuses all but `"Baked"`, with a console warning) |
| `Light.GetAreaSize` / `SetAreaSize` | `(id)` / `(id, width, height)` | `width, height` in world units (default `1, 1`; each side floored at `0.01`; a NaN or `math.huge` side refuses the whole size, keeping the current one; `0, 0` with no light) |

`SetType` and `SetMode` are case-insensitive; an unrecognized name is ignored (the
current value is kept).

**Falloff** (#1699). A point, spot or Area light fades as Unity URP's and
Frostbite's *windowed inverse-square*: at distance `d` from the lamp it delivers
`intensity × saturate(1 − (d/range)⁴)² / max(d², 0.0001)`. Close in that is plain
inverse-square, and it reaches exactly zero at `range` with no visible edge, so
`range` is how far the pool of light reaches, not where it is cut. A spotlight's
cone is URP's squared ramp: full inside `inner_cone`, `t²` between the cones
(`t` running 0 at `outer_cone` to 1 at `inner_cone`, in cosines), nothing outside.
The forward shader, the lightmap bake and the light-probe bake all use this one
curve. Before #1699 a light fell off as `1 / (d² + 1)` and was cut hard at its
range, which left a bright ring at the edge. The new curve is brighter close in,
`1/d²` instead of `1/(d² + 1)` (twice as bright at 1 m), and dimmer toward the edge.
Where the two cross depends on the range: for a range of 4.4 the new curve is 1.15×
the old at 2 m and drops below it past about 2.35 m. An existing scene's lamps want
their intensity re-tuned, and a rebake; nothing is rescaled for you.
Inverse-square has no ceiling close in, so four places bound it: a lit particle
floors the distance at half its sprite's size; an Area light's stand-in in a probe
or reflection capture floors it at `√(area / π)`, which gives the panel's true
`π × intensity` up against it; the lightmap bake clamps each bounce path's light
at 10 (Blender Cycles' *Clamp Indirect* default), never a texel's own direct
light; and a drawn pixel is held below half-float's limit (60000), so bloom never
reads an infinity.

**Area lights** (#1674) are Unity's *Area (Rectangle)* light, and like Unity's
built-in pipeline rusty bakes them only: an Area light acts as
`Baked` whatever its stored mode (`GetMode` and `Component.Get` read `"Baked"`, and
`SetMode` refuses the rest, warning in the console). The stored mode is kept, so a
Mixed point light switched to Area and back is Mixed again. The
emitter is a `width × height` rectangle (`SetAreaSize`) centred on the entity,
spanning its local X and Y and shining from its front, local −Z, only. Intensity is
per unit area: a 2 × 1 panel gives out twice what a 1 × 1 one of the same
intensity does, and seen from far away it acts as a spot light of `intensity ×
area` whose light fades as the cosine off the panel's facing. Its light fades to zero
at `range`, measured from its centre, so no receiver that far from the centre is lit,
like a point light's. The lightmap bake integrates the whole
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
light outside the view costs nothing. One whose intensity or colour is zero (a
blinking alarm in its off half: #1712) is listed in no cluster, so no pixel shades
it; it still counts towards the camera's budget of 64 (by its range, as any light
does) and, if it casts shadows, it may still take atlas tiles and draw its
casters. A light's **range** is where its light reaches zero, what the camera's budget ranks it
by, and how far it reaches into the cluster grid, so a tight range is the cheap
one; a spotlight is listed only in the clusters its cone touches, so a narrow beam
costs fewer pixels than a point light of the same range. Up to **4** directional lights
shade at once; the last active one is the sun that casts the cascaded shadows. Only
the last active ambient light counts: it replaces the scene's ambient, a `Gradient`
one included, with its single colour (`Lighting.md`, *Environment*).

**Shadows.** The sun (slot 0 above) always casts its cascaded shadows. A `Baked`
sun is not drawn live, so it is no sun here: outside a bake capture, which does draw
it live and shadow it, a scene with no live directional light fits, draws and samples
no cascade at all (#1713). A point or
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
its tiles each frame (`shadow_atlas_cached` / `shadow_atlas_rebaked` count it), and
a tile no dynamic caster reaches is not redrawn at all. A light **keeps its tiles**
from frame to frame (#1711, HDRP's cached shadow atlas): one blinking to intensity
0, leaving the view or losing the light budget keeps them for 600 rendered frames,
so it comes back with its bake; a tile changes size only after the light's wanted
size has been out of its band for 15 frames in a row (a quarter of a step past
either end of its own step, `[0.75 × size, 2.25 × size)`, never past the largest
tile, so a light the camera walks into always reaches it). A tile the atlas had to
shrink to fit grows back as soon as the room is there. A moving or
turning light re-bakes every frame it moves, and so does a light whose tiles the
atlas has to repack (a new light that the free space could hold only once
rearranged).
The atlas holds sixteen 512² tiles, so one point light at full size takes
six of them. Every caster shadows
into the atlas as it does into the cascades: cutout and dissolving materials clip
their shadow, skinned meshes cast in their animated pose. Lit particles are not
shadowed. **Shadow fade distance** (`SetShadowFadeDistance`, HDRP's *Shadow Fade
Distance*, #1712) stops a far light's shadow from being sampled: over the last tenth
of that distance between the camera and the light the shadow fades out, and past it
the light shades unshadowed. `0`, the default, never fades. The light keeps its
atlas tiles either way.

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
included. Its neighbour on the Mesh card, `scale_in_lightmap` (Unity's *Scale In
Lightmap*), scales the mesh's lightmap texel density; at `0` the mesh reads light
probes instead ([`Lighting`](Lighting.md#generate-lighting)). Whether a surface *receives* shadows is its material's switch,
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
| `Mixed` *(default)* | live direct light + baked bounce (+ its baked static shadow under `Shadowmask`) | live direct light + bounce from the probes (+ the probes' baked occlusion under `Shadowmask`) | yes, if it casts shadows |
| `Baked` | direct light and bounce from the lightmap, and one specular highlight from a directional lightmap | direct light and bounce from the probes only, no highlight | no |

- `Realtime` — direct light and shadows every frame; neither bake sees it, so it
  bounces nowhere (a muzzle flash, a flickering lamp).
- `Mixed` — realtime direct light (diffuse and specular) and shadows, plus its
  bounce baked into the lightmaps and the light probes. What else it bakes is the
  scene's **Mixed Lighting Mode** (`Lighting.SetSettings{ mixedLightingMode = … }`,
  Unity's *Lighting Mode*): under `"BakedIndirect"` (the default, Unity's *Baked
  Indirect*) nothing, so its shadow ends at the shadow distance. Under
  `"Shadowmask"` (#1695) a shadow-casting `Mixed` light (the sun always) also bakes
  its **static occlusion**: a shadowmask page beside each lightmap page for static
  surfaces, and one value per light probe for dynamic objects. Its static shadows
  then hold at any distance, it still glints on glossy surfaces, and a dynamic
  enemy under it still casts a realtime shadow. The light that should cast sharp
  shadows on actors. Details below.
- `Baked` — costs nothing at runtime: it is never drawn live, anywhere. Static
  surfaces get it from their lightmap; dynamic objects get its direct light and its
  bounce from the light probes, whose bake adds the light's own direct light per
  probe (shadowed by static geometry). So a dynamic object lights from it only once
  probes are baked, softly, and it casts no realtime shadow; a static surface without
  a lightmap (no second UV map) does not receive it at all, while a static mesh at
  Scale In Lightmap `0` ([`Lighting`](Lighting.md#generate-lighting)) takes it from the
  probes, as a dynamic object does.

  **Its highlight** (#1695 step 1). A lightmap holds a `Baked` light's diffuse light only, so
  on its own a metal rail or a polished floor beside a `Baked` lamp shows no glint.
  A **directional** bake (`Lighting.BakeLightmaps`'s `directional`, on by default)
  also records where each texel's light comes from and how much of it is `Baked`
  lights' direct light (its *lamp share*), and a lightmapped surface turns that part
  into one GGX highlight from the lamps' direction, as a live light's, with the
  lamps' share of the texel's light as its strength. Unity has no such term; it
  follows Bakery's *Directional Specular* and Frostbite. It is always on with a
  directional bake (a per-material switch would sit with specular AA's), costs one
  BRDF evaluation per lightmapped pixel, and is not darkened by SSAO, which leaves
  direct light alone (the static occluders' shadows are already in the bake). Its
  limits:
  - **Lamps only.** Sky, bounce and emission give no highlight, however much they
    come from one side (an open sky over a dark ground, a `Mixed` spot's bounce off
    the floor onto a glossy ceiling): the environment reflection reflects them. A
    `Mixed` light gives its highlight live, and its baked bounce none. A lamp
    supplying under a quarter of a texel's light (the fringe of its range, where a
    bounce outshines it) has its highlight faded with its share, to none at none.
  - **One highlight per texel.** Two lamps lighting one texel give one highlight
    between them, pulled toward both and weakened (two lamps at 45° either side of
    a floor's normal keep about 0.7 of their light in it), where live lights give
    two. Where a lamp supplies under a quarter of a texel's light (the edge of its
    range), the highlight's direction eases from the lamp's toward the mean of all
    the texel's light, and the highlight fades with it, so the hand-over leaves no
    seam and no highlight where no light comes from.
  - **An Area light gives a dot, not a panel.** A `Baked` Area light's highlight is
    one tight lobe toward its centre, and it weakens where the panel fills the
    texel's view (its light arrives from many directions there).
  - **Grazing light weakens it.** A lamp within about 6° of a surface's plane gives
    a fraction of its live highlight (a lamp 0.5 m over a polished floor, 8 m off,
    about 62%; a sun 2° up, about 35%).
  - **A two-sided surface's back face** shows its front's baked light (the bake
    lights the front only) and no highlight.
  - **Its sharpness is the live one's.** The direction page is stored uncompressed
    (RGBA8), so between texels a lamp's direction is rebuilt to well under a degree,
    and a polished surface's highlight is as round and as sharp baked as live, at 8
    texels per unit as at 32 (#1996). What a texel cannot hold is light that changes
    inside it: a shadow's or a cone's edge across a texel is blurred over it, in the
    highlight as in the diffuse light; raise the bake's texels per unit where that
    matters.
  - **A level needs a directional bake** to have it; a non-directional lightmap, a
    surface with no lightmap and a probe-lit dynamic object get none. No probe or
    reflection bake sees it, since it depends on the viewer.

**Shadowmask** (#1695, Unity's *Shadowmask* and *Distance Shadowmask*). With the
scene's Mixed Lighting Mode at `"Shadowmask"`
([`Lighting`](Lighting.md#generate-lighting)), the next bake gives each `Mixed` light
that casts shadows (and every `Mixed` directional light, which always does) one of
four **channels** (Unity's `Light.bakingOutput.occlusionMaskChannel`) and stores, per
lightmap texel and per light probe, how much of that light reaches it past static
occluders. In the frame its direct light, diffuse and specular, stays live; its
shadow is the baked one met with the realtime one as the running quality preset's
`shadowmask_mode` says ([`Graphics`](Graphics.md#quality-presets)):

- `"Shadowmask"` — only dynamic casters draw the light's realtime shadow, out to its
  shadow distance (the sun's cascades; a point or spot light's own shadow fade
  distance); the static shadow is the mask's at every distance, and the darker of
  the two wins. Static casters stay out of its shadow maps, so they cost nothing
  there, with two exceptions where the maps keep them and the light draws as under
  `"DistanceShadowmask"`: while **volumetric fog** is on (a camera's volume turns it
  on), since the fog's air reads only the shadow maps and would otherwise glow
  through walls, so a fogged scene pays for its static casters there; and until the
  shadowmask pages are **bound** (still decoding, a missing or refused PNG, or a bake
  that recorded channels and wrote no page), so a load failure leaves the scene
  shadowed.
- `"DistanceShadowmask"` — every caster draws its realtime shadow within the shadow
  distance, fading into the mask past it, as in Unity: no pop at the edge, only the
  realtime shadow's softness giving way to the mask's.

What to know:

- **Four to a texel.** Channels are an overlap colouring of the lights' reach (a
  point or spot light's range sphere; a directional light meets every light), so at
  most four masked lights overlap anywhere. A light that finds all four taken **falls
  back to `Baked`** in the bake's record (`GetMode` still reads `"Mixed"`): its direct
  light goes into the lightmaps and probes, the frame stops drawing it live, and the
  bake names it once in the console. So a level switches to `Mixed` only the lamps
  that matter (the ones over glossy surfaces and in the actors' paths) and leaves the
  rest `Baked`: each `Mixed` lamp takes a place in the frame's light loop, and the
  clustered budget drops what is over it per cluster, as it always did.
- **Past the shadow atlas's budget** a masked light still has its baked static
  shadow; only its dynamic casters' shadow is missing that frame.
- **What the mask holds.** A texel's channel is one shadow ray to the light, 0 or 1,
  filtered by the page's mips and bilinear sampling, so the baked shadow's edge is as
  sharp as the lightmap's texels. A probe's is the same ray from the probe,
  interpolated at the object's position, so a dynamic object's baked shadow is soft
  and whole-object. A static surface with no lightmap (no second UV map) reads the
  probes' too; a lightmapped one whose shadowmask pages have not loaded yet has the
  realtime shadow alone until they do, from maps that then hold the static casters
  (above), faded out past the shadow distance.
- **Two-sided and translucent panels.** A static panel lit from behind by a `Mixed`
  lamp reads mask 0 on its back-lit side: the bake's shadow ray starts on the front
  of the surface's normal and meets the panel itself. Light it with a `Baked` or
  `Realtime` lamp, or face the panel toward the lamp.
- **Outside every probe.** Under plain `"Shadowmask"` a dynamic object (or a static
  one with no lightmap) outside every light probe reads occlusion 1 and the maps hold
  no static casters, so it gets no static shadow from a masked light at all, as in
  Unity. Cover the level with probes where actors walk.
- **A mode change takes effect next frame.** A masked lamp set to `Realtime` or its
  shadows turned off is masked by no channel; a fallen-back lamp set to `Realtime`
  draws live again.
- **A light moved or retuned after the bake** keeps its old mask, as in Unity: its
  static shadow stays where it was baked until the next bake.
- **Switching back to `"BakedIndirect"`** draws as a `BakedIndirect` scene at once
  (the mask is ignored); a light that fell back stays `Baked` until the next bake.
- **Bake captures** (the light and reflection probes) draw a `Mixed` light as
  `BakedIndirect` does: shadowed by realtime maps that hold every static caster.
- **Costs.** One RGBA8 shadowmask page with mips per lightmap page (5.25 MiB on the
  GPU for a 1024² page, with its three levels), one `vec4` per probe in the lighting sidecar, and in the
  bake one more shadow ray per masked light per texel and per probe.
