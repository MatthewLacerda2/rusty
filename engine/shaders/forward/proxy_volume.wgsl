// Light Probe Proxy Volumes (Unity's `LightProbeProxyVolume`): a dynamic object's probe light
// interpolated per pixel over a grid of probe samples spanning its bounds, instead of the one
// sample at its world bounds' centre (`eval_sh`).

#define_import_path forward::proxy_volume

#import common::PI
#import forward::bindings::proxy_volumes
#import lighting::sh::sh_basis

// `render::draw::proxy_volume::pack`'s layout, in vec4s: the world-to-box matrix's columns (box
// [0, 1]^3) and the grid's dims, then each point's L1 SH, x fastest. Change both together.
const HEADER: u32 = 5u;
const COEFFS: u32 = 4u;

fn add_point(acc: ptr<function, array<vec3<f32>, 4>>, base: u32, p: u32, w: f32) {
    let first = base + HEADER + p * COEFFS;
    for (var c = 0u; c < COEFFS; c = c + 1u) {
        (*acc)[c] += proxy_volumes[first + c].xyz * w;
    }
}

// The irradiance E at `world_pos` for normal `N` from proxy volume `proxy` (an instance's
// `proxy_volume`), in rgb, with a = 1; a = 0 when it has none, and the caller keeps the single
// sample.
fn proxy_irradiance(world_pos: vec3<f32>, N: vec3<f32>, proxy: u32) -> vec4<f32> {
    if (proxy == 0u) {
        return vec4<f32>(0.0);
    }
    let base = proxy - 1u;
    let to_box = mat4x4<f32>(
        proxy_volumes[base], proxy_volumes[base + 1u],
        proxy_volumes[base + 2u], proxy_volumes[base + 3u],
    );
    let uvw = clamp((to_box * vec4<f32>(world_pos, 1.0)).xyz, vec3<f32>(0.0), vec3<f32>(1.0));
    let dims = vec3<u32>(proxy_volumes[base + 4u].xyz);
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
    let b = sh_basis(N);
    let band1 = 2.0 * PI / 3.0;
    let e = acc[0] * (b[0] * PI)
        + (acc[1] * b[1] + acc[2] * b[2] + acc[3] * b[3]) * band1;
    return vec4<f32>(max(e, vec3<f32>(0.0)), 1.0);
}
