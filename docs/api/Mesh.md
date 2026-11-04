## `Mesh`

Per-renderer settings of an entity's `MeshComponent` (#1714, `src/api/mesh.rs`):
Unity's *Mesh Renderer*. Today they are the **small-object cull**: a prop that is
too far away, or too small on screen, is not drawn. A 20 cm can on a shelf across
a 50 m hall covers a few pixels: drawing it costs a draw, its shadow and its
lighting, and shows nothing. Both settings are off by default, so a scene that sets
neither draws exactly what it drew before.

| Function | Arguments | Returns |
|---|---|---|
| `Mesh.GetMaxDrawDistance` | `(id)` | the distance in metres past which a camera stops drawing it; `0` = unlimited |
| `Mesh.SetMaxDrawDistance` | `(id, metres)` | — negative, NaN or infinite is `0` |
| `Mesh.GetMinScreenSize` | `(id)` | the screen-relative height (`0..1`) under which a camera stops drawing it; `0` = off |
| `Mesh.SetMinScreenSize` | `(id, height)` | — held to `[0, 1]`; NaN or infinite is `0` |

Without a Mesh, getters return `0` and setters do nothing. The same fields are
`max_draw_distance` and `min_screen_size` to `Component.Get` / `Component.Set(id,
"Mesh", { … })` ([`Component`](Component.md)), the Inspector's Mesh card shows them,
and the scene and its prefabs save them when set.

**Max Draw Distance** is Unreal's: measured from the camera to the centre of the
mesh's world bounds. **Min Screen Size** is the measure Unity's `LODGroup` picks
levels by ([`LODGroup`](LODGroup.md)), and so the same value as its *Culled*
share: the mesh's size over the height the view spans at that distance,
`size / (2 · distance · tan(fov / 2))`; an orthographic camera measures
`size / (2 · half-height)` at any distance. The size is fixed per mesh, as a
group's is: the largest extent of its local bounds times its transform's largest
axis scale, so rotating it never changes the verdict (a skinned mesh is sized by
its bind pose, not by the pose it is animated into). A mesh past either is culled;
with both set, whichever culls first.

- **Per camera, per frame.** Each camera measures from itself: the main view, a
  stacked camera, a render-texture camera. Whether a mesh was culled is the
  renderer's and is not readable from scripts: the simulation never depends on it.
  A culled mesh still collides, animates and runs its scripts.
- **Shadows go with the prop, as the main camera sees it.** Shadow maps are drawn
  once per frame, measured from the **main (base) camera**, never the light: a prop
  that camera culls casts nothing, and one it draws keeps its shadow, so the shadow
  never pops on its own. Another camera (a stacked one, a render-texture CCTV
  screen) has its own verdict for the prop but shares those shadows, so it can
  show a prop the main camera culled, with no shadow under it. A mesh with either
  setting is never baked into the cached static shadows (the sun's static
  cascades, a light's static atlas tiles): it casts from the per-frame shadow pass
  even when Static, so its shadow can follow the camera. Turning a setting on or
  off re-bakes them. A `ShadowsOnly` mesh (no camera draws it,
  [`Light`](Light.md#cast-shadows)) is measured the same way for its shadow.
- **LODGroup.** A level its group hides is skipped before this test; a level the
  group shows is still culled by its own renderer's settings, whichever culls
  first. When any renderer of a group sets either, **every** renderer of that
  group leaves the static shadow bake, Static or not: the bake draws a group at
  LOD0 only, so a group split between the bake and the per-frame pass would leave
  a level with no shadow, or with two.
- **Bakes see everything.** Light-probe, reflection-probe and lightmap captures
  cull nothing by these settings: a probe on the far side of a prop still sees it.
- **The cost.** The test runs only on meshes the frustum kept: a branch with both
  settings off, a distance and a division with one on, nothing for what the
  frustum culled. A culled mesh is never drawn, in a camera view or a shadow map,
  but the shadow pass still gathers it each frame to test it (its material runs
  and one entry per run; a skinned one uploads no bone palette). A prop pops in
  and out at the threshold; there is no fade.

`Debug.Stats()` counts it per frame ([`Debug`](Debug.md)): `cull_tested`,
`culled_entities` and `size_culled_entities` over the camera views, and
`shadow_cull_tested`, `shadow_frustum_culled` and `shadow_size_culled` over the
shadow views.

```lua
-- A can on a shelf: gone past 30 m, or under 2% of the screen.
local can = Scene.CreateEntity("Can", "Cylinder")
Mesh.SetMaxDrawDistance(can, 30)
Mesh.SetMinScreenSize(can, 0.02)
print(Mesh.GetMaxDrawDistance(can)) --> 30.0
```

The values are stored as 32-bit floats, so `Mesh.GetMinScreenSize(can)` reads back
`0.019999999552965`, not `0.02`: compare with a tolerance, never with `==`.
