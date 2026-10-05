## `Layers`

An entity's layer is a single index (`0..31`) into the project's shared Layers
registry — Unity's per-object layer. Layer 0 is the fixed `"Default"`; the names
are managed in the **Tags & Layers** section of the Scene Settings panel and
persist with the scene.

A layer drives three things:

- **Collisions.** The **Layer Collision Matrix** (Scene Settings) decides which
  layers physically collide: two colliders on layers whose cell is off never
  touch, and a `CharacterController` walks through them. Changing an entity's
  layer during Play takes effect on the next physics step. `Physics.GenerateHitboxes`
  puts hitboxes on a `Hitbox` layer whose matrix row starts all off, so they only
  answer queries (see [Physics](Physics.md#per-bone-hitboxes-464)).
- **Queries.** Every `Physics` cast, overlap and check takes an optional
  `layer_mask` (one bit per layer, e.g. `1 << Layers.NameToIndex("Enemy")`) and
  reports only entities on those layers. This is independent of the matrix: a
  layer that collides with nothing is still hit by a query whose mask includes it.
- **Rendering.** A camera's **culling mask** (the Camera card's checklist) skips
  meshes, particles and trails on the layers it leaves out. This is how a stacked
  viewmodel camera draws only the gun and the world camera everything else.

All three are scriptable (#827), with Unity's names and placement: a slot's name
with `Layers.SetName`, the matrix with
[`Physics.IgnoreLayerCollision`](Physics.md#layer-collision-matrix-827), a camera's
mask with [`Camera.SetCullingMask`](Camera.md#camera-entities-projection-and-render-textures-430).
They write the same values as the editor and persist with the scene.

```lua
-- A viewmodel: the gun on its own layer, drawn only by the stacked gun camera.
Layers.SetName(8, "Viewmodel")
local vm = Layers.NameToIndex("Viewmodel")
Layers.SetLayer(gun, vm)
Camera.SetCullingMask(worldCam, ~(1 << vm))
Camera.SetCullingMask(gunCam, 1 << vm)
```

| Function | Signature | Returns |
|---|---|---|
| `Layers.GetLayer` | `(id)` | layer index (`0` if the entity is missing) |
| `Layers.SetLayer` | `(id, index)` | — |
| `Layers.GetName` | `(index)` | the slot's name, or `Layer N` if unnamed |
| `Layers.SetName` | `(index, name)` | — (slot 0 stays `"Default"`: a no-op; a blank name clears the slot; errors outside `0..31`) |
| `Layers.NameToIndex` | `(name)` | layer index, or `nil` if unknown |
