## `Decals`

Stamp **box-projector decals** (bullet holes, scorch, blood splats) onto the
surface a shot hit. A decal is a projected *volume*, not a flat sticker: the decal
pass reconstructs the underlying surface from the scene depth and wraps the texture
onto whatever geometry the box overlaps. Spawn it from the hit point and surface
normal the cast already returns. The registry is a bounded FIFO — oldest decals
are evicted past the cap.

```lua
local hit, id, dist, px, py, pz, nx, ny, nz = Physics.Raycast(ox,oy,oz, dx,dy,dz, self_id)
if hit then Decals.Spawn(px, py, pz, nx, ny, nz) end
```

| Function | Signature | Returns |
|---|---|---|
| `Decals.Spawn` | `(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])` | — |
| `Decals.Count` | `()` | live decal count |
| `Decals.Clear` | `()` | — (drops every live decal) |

`size` (default `0.5`) is the stamp's width/height in world units; `texture` is the
decal sprite path (default checker); `rotation_deg` spins the stamp around its
projection axis; `r,g,b,a` tints the texel (default opaque white).
