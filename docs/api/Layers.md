## `Layers`

An entity's layer is a single index (`0..31`) into the project's shared Layers
registry — Unity's per-object layer. Layer 0 is the fixed `"Default"`; the names
are managed in the **Tags & Layers** section of the Scene Settings panel and
persist with the scene. This is groundwork: a layer carries no collision or
rendering behaviour on its own yet (that arrives with the collision matrix and
camera culling masks).

| Function | Signature | Returns |
|---|---|---|
| `Layers.GetLayer` | `(id)` | layer index (`0` if the entity is missing) |
| `Layers.SetLayer` | `(id, index)` | — |
| `Layers.GetName` | `(index)` | the slot's name, or `Layer N` if unnamed |
| `Layers.NameToIndex` | `(name)` | layer index, or `nil` if unknown |
