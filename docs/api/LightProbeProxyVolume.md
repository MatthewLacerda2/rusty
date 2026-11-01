## `LightProbeProxyVolume`

Lights a dynamic object from a **grid of light-probe samples over its bounds**,
interpolated per pixel, instead of from one sample at its origin: Unity's
`LightProbeProxyVolume` (#1672). A 2 m locker under a wall lamp, a bunk bed, a grunt
walking under a light: without a volume the whole mesh takes one colour, so the
locker's foot is as bright as its top; with one, the top is lit and the foot dark.
It is off by default; the single sample stays the cheap path.

The volume lights the **mesh on the same entity**, when that mesh is lit by probes
at all (a non-static object, see [`Probe`](Probe.md); a lightmapped static surface
reads its lightmap). Each grid point holds what the probe field gives at that
point (the same interpolation the single sample uses), and a pixel inside the box
blends its eight nearest points; a pixel outside the box (a mesh larger than a
custom box, an arm swung past the bind-pose bounds) reads the box's nearest face,
as Unity clamps, so there is no seam where the box ends. A volume that would not
fit in a frame's storage binding (dozens of visible high-resolution ones at once)
is left out of that frame, and its object keeps the single sample (the console
says so once). The grid is sampled again whenever the
object moves, its settings or mesh change, or the probes are re-baked (Unity's
*Automatic* refresh), and never while all of those stand still. Only the settings
below are saved, never the samples.

**The box** (`BoundingBoxMode`): `AutomaticLocal` (the default) spans the mesh's own
bounds, in the entity's local space, so it moves, turns and scales with the
`Transform`; `Custom` spans `Size` around `Origin`, also local.

**The resolution** (`ResolutionMode`): `Automatic` (the default) puts
`ProbeDensity` points per metre along each axis of the box (from `0.1` to `100`,
default `1`); `Custom` takes `GridResolution`, a count per axis. Either way each
axis is rounded up to a power of two from `1` to `32`, as Unity does, so a 2 m
locker at the default density is `2` points tall. Points sit at the centres of the
box's cells.

**The quality** (`Quality`): `Normal` (alias `L1`, the default) keeps each point's
colour and the direction it comes from (SH bands 0 and 1); `Low` (alias `L0`) keeps
one colour per point, the same from every direction. Inside a volume the object
loses the single sample's L2 detail, as in Unity.

Without a volume a getter returns `nil` (zeros for `GetSize`, `GetOrigin` and
`GetGridResolution`), and a setter is an error naming the entity. Add one with
`Scene.AddComponent(id, "LightProbeProxyVolume")` (aliases `ProxyVolume`, `LPPV`);
`Component.Get/Set` reach every field by its snake_case name. `Debug.Snapshot`
reports it as `light_probe_proxy_volume`.

```lua with=locker:Mesh+LightProbeProxyVolume
LightProbeProxyVolume.SetResolutionMode(locker, "Custom")
LightProbeProxyVolume.SetGridResolution(locker, 1, 8, 1)
LightProbeProxyVolume.SetQuality(locker, "Normal")
```

| Function | Signature | Returns |
|---|---|---|
| `LightProbeProxyVolume.GetBoundingBoxMode` / `SetBoundingBoxMode` | `(id)` / `(id, name)` | `"AutomaticLocal"` or `"Custom"` (an unknown name is an error listing them) |
| `LightProbeProxyVolume.GetSize` / `SetSize` | `(id)` / `(id, x, y, z)` | the custom box's size, local; each side at least `0.001` (non-finite is ignored) |
| `LightProbeProxyVolume.GetOrigin` / `SetOrigin` | `(id)` / `(id, x, y, z)` | the custom box's centre, local (non-finite is ignored) |
| `LightProbeProxyVolume.GetResolutionMode` / `SetResolutionMode` | `(id)` / `(id, name)` | `"Automatic"` or `"Custom"` |
| `LightProbeProxyVolume.GetProbeDensity` / `SetProbeDensity` | `(id)` / `(id, density)` | points per metre under `Automatic`, `0.1`–`100` (non-finite is ignored) |
| `LightProbeProxyVolume.GetGridResolution` / `SetGridResolution` | `(id)` / `(id, x, y, z)` | points per axis under `Custom`, each rounded up to a power of two from `1` to `32` |
| `LightProbeProxyVolume.GetQuality` / `SetQuality` | `(id)` / `(id, name)` | `"Normal"` (L1) or `"Low"` (L0) |
