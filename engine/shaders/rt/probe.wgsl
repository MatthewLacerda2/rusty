// `render::rays::probe`: traces given rays (the tests) or one ray per pixel (the
// measurement) through `rt::trace`, and writes what each hit.

#import rt::trace::{trace_closest, trace_any, Hit}

struct ProbeRay {
    origin: vec4<f32>,
    dir: vec4<f32>,
};

struct ProbeHit {
    t: f32,
    slot: u32,
    tri: u32,
    any: u32,
    bary: vec2<f32>,
    pad: vec2<f32>,
};

struct Eye {
    inv_view_proj: mat4x4<f32>,
    origin: vec4<f32>,
    size: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> rays: array<ProbeRay>;
@group(0) @binding(1) var<storage, read_write> hits: array<ProbeHit>;
@group(0) @binding(2) var<uniform> eye: Eye;
@group(0) @binding(3) var<storage, read_write> depths: array<f32>;
@group(0) @binding(4) var<storage, read_write> pixel_hits: array<ProbeHit>;

@compute @workgroup_size(64)
fn cs_rays(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= arrayLength(&rays) {
        return;
    }
    let ray = rays[i];
    let hit = trace_closest(ray.origin.xyz, ray.dir.xyz, ray.origin.w);
    let any = trace_any(ray.origin.xyz, ray.dir.xyz, ray.origin.w);
    hits[i] = ProbeHit(hit.t, hit.slot, hit.tri, u32(any), hit.bary, vec2<f32>(0.0));
}

fn pixel_ray(id: vec2<u32>) -> Hit {
    let uv = (vec2<f32>(id) + 0.5) / vec2<f32>(eye.size.xy);
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let at = eye.inv_view_proj * vec4<f32>(ndc, 0.5, 1.0);
    let dir = normalize(at.xyz / at.w - eye.origin.xyz);
    return trace_closest(eye.origin.xyz, dir, 1e6);
}

// The lean measurement: 4 bytes a pixel, the hit distance (-1 for a miss).
@compute @workgroup_size(8, 8)
fn cs_camera(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= eye.size.x || id.y >= eye.size.y {
        return;
    }
    let hit = pixel_ray(id.xy);
    depths[id.y * eye.size.x + id.x] = select(-1.0, hit.t, hit.tri != 0xffffffffu);
}

// The full measurement: the whole hit a pixel, as a reflection pass reads it.
@compute @workgroup_size(8, 8)
fn cs_camera_full(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= eye.size.x || id.y >= eye.size.y {
        return;
    }
    let hit = pixel_ray(id.xy);
    pixel_hits[id.y * eye.size.x + id.x] = ProbeHit(hit.t, hit.slot, hit.tri, 0u, hit.bary, vec2<f32>(0.0));
}
