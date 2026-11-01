// engine/shaders/forward/proxy_volume.wgsl — Light Probe Proxy Volumes (#1672,
// Unity's `LightProbeProxyVolume`): a dynamic object's probe light interpolated per
// pixel over a grid of probe samples spanning its bounds, instead of the one sample
// at its origin (`eval_sh`).
//
// An instance's `proxy_volume` is 0 for none, else the first vec4 of its volume in
// `proxy_volumes` plus one (`render::draw::proxy_volume::pack`). A volume is:
//   4 vec4  the world -> box matrix's columns (the box is [0, 1]^3),
//   1 vec4  its grid dimensions (u32 bits in xyz),
//   then 4 vec4 per grid point (x fastest, then y, then z): its L1 SH, RGB in xyz
//   (bands 1..3 zeroed at `Low` quality).
// Points sit at cell centres, as a 3D texture's texels do, and a lookup clamps to
// the outer ones the way a texture's clamp-to-edge does.

#define_import_path forward::proxy_volume

#import common::PI
#import forward::bindings::{instances, proxy_volumes}

const HEADER: u32 = 5u;
const COEFFS: u32 = 4u;
// How far past its box a pixel still reads the volume, in box units: a surface
// on the box's face must not flicker between the grid and the single sample.
const EDGE: f32 = 0.001;

// The L1 SH of point `p` of the volume whose header starts at `base`, scaled by `w`
// into `acc`.
fn add_point(acc: ptr<function, array<vec3<f32>, 4>>, base: u32, p: u32, w: f32) {
    let first = base + HEADER + p * COEFFS;
    for (var c = 0u; c < COEFFS; c = c + 1u) {
        (*acc)[c] += proxy_volumes[first + c].xyz * w;
    }
}

// The irradiance E at `world_pos` for normal `N` from instance `instance`'s proxy
// volume, in rgb, with a = 1; a = 0 when it has none or the point is outside its
// box, and the caller keeps the single sample. The L1 terms are `eval_sh`'s first
// four, so a one-point volume reads what `eval_sh` reads without its L2 bands.
fn proxy_irradiance(world_pos: vec3<f32>, N: vec3<f32>, instance: u32) -> vec4<f32> {
    let proxy = instances[instance].proxy_volume;
    if (proxy == 0u) {
        return vec4<f32>(0.0);
    }
    let base = proxy - 1u;
    let to_box = mat4x4<f32>(
        proxy_volumes[base], proxy_volumes[base + 1u],
        proxy_volumes[base + 2u], proxy_volumes[base + 3u],
    );
    let uvw = (to_box * vec4<f32>(world_pos, 1.0)).xyz;
    if (any(uvw < vec3<f32>(-EDGE)) || any(uvw > vec3<f32>(1.0 + EDGE))) {
        return vec4<f32>(0.0);
    }
    let dims = bitcast<vec4<u32>>(proxy_volumes[base + 4u]).xyz;
    let last = vec3<f32>(dims - vec3<u32>(1u));
    let g = clamp(uvw * vec3<f32>(dims) - 0.5, vec3<f32>(0.0), last);
    let g0 = vec3<u32>(floor(g));
    let g1 = min(g0 + vec3<u32>(1u), dims - vec3<u32>(1u));
    let f = g - floor(g);
    var acc: array<vec3<f32>, 4>;
    for (var corner = 0u; corner < 8u; corner = corner + 1u) {
        let hi = vec3<bool>((corner & 1u) != 0u, (corner & 2u) != 0u, (corner & 4u) != 0u);
        let at = select(g0, g1, hi);
        let w3 = select(vec3<f32>(1.0) - f, f, hi);
        let p = (at.z * dims.y + at.y) * dims.x + at.x;
        add_point(&acc, base, p, w3.x * w3.y * w3.z);
    }
    // `eval_sh`'s basis and cosine lobes, bands 0 and 1.
    let band1 = 0.4886025 * 2.0 * PI / 3.0;
    let e = acc[0] * (0.2820948 * PI)
        + acc[1] * (-band1 * N.y)
        + acc[2] * (band1 * N.z)
        + acc[3] * (-band1 * N.x);
    return vec4<f32>(max(e, vec3<f32>(0.0)), 1.0);
}
