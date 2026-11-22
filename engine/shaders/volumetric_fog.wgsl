// Frostbite's unified volumetrics (Hillaire 2015). `cs_scatter`: per froxel, the medium
// at a jittered point lit by ambient, suns, clustered lights through their shadow tiles
// and `Baked` lamps unshadowed, through Henyey-Greenstein, blended with last frame's
// froxel reprojected; rgb light per metre, a extinction. `cs_integrate`: per column,
// front to back (Frostbite's energy-conserving step), so texel k holds the eye to slice
// k's far side.

#import common::{CameraUniforms, LightingUniforms, LocalLight, NO_SHADOW, ShadowTile, cluster_index, local_light_radiance, local_shadow_tile, sample_local_shadow, PI}
#import fog::volumetric::{froxel_depth, froxel_slice}
#import lighting::ambient::ambient_average
#import lighting::cascades::ShadowCascades
#import numeric::{finite3, finite4}

struct Params {
    prev_view_proj: mat4x4<f32>,
    prev_view_z: vec4<f32>,
    prev_volumetric: vec4<f32>,
    grid: vec4<u32>,
    jitter: vec4<f32>,
    // x extinction at the base height (1 / mean free path), y base height, z falloff
    // per metre above it, w anisotropy (g).
    medium: vec4<f32>,
    albedo: vec4<f32>,
    probes_min: vec4<f32>,
    probes_size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<uniform> camera: CameraUniforms;
@group(0) @binding(2) var<uniform> lighting: LightingUniforms;
@group(0) @binding(3) var<storage, read> local_lights: array<LocalLight>;
@group(0) @binding(4) var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(0) @binding(5) var<storage, read> cluster_lights: array<u32>;
// Baked lamps reach surfaces through the lightmaps, the air only through here,
// unshadowed.
@group(0) @binding(6) var<storage, read> baked_lights: array<LocalLight>;
@group(0) @binding(7) var<storage, read> baked_ranges: array<vec2<u32>>;
@group(0) @binding(8) var<storage, read> baked_indices: array<u32>;
@group(0) @binding(9) var<uniform> shadow: ShadowCascades;
@group(0) @binding(10) var t_shadow: texture_depth_2d_array;
@group(0) @binding(11) var s_shadow: sampler_comparison;
@group(0) @binding(12) var t_shadow_atlas: texture_depth_2d;
@group(0) @binding(13) var<storage, read> shadow_tiles: array<ShadowTile>;
@group(0) @binding(14) var t_history: texture_3d<f32>;
@group(0) @binding(15) var s_history: sampler;
@group(0) @binding(16) var out_scatter: texture_storage_3d<rgba16float, write>;
@group(0) @binding(17) var t_probes: texture_3d<f32>;

// As `forward::transparent::MAX_HDR`, so every reader gets a finite value.
const MAX_FROXEL: f32 = 60000.0;

fn phase(g: f32, c: f32) -> f32 {
    let g2 = g * g;
    return (1.0 - g2) / (4.0 * PI * pow(max(1.0 + g2 - 2.0 * g * c, 1e-4), 1.5));
}

fn view_ray(uv: vec2<f32>) -> array<vec3<f32>, 2> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let a4 = camera.inv_view_proj * vec4<f32>(ndc, 0.25, 1.0);
    let b4 = camera.inv_view_proj * vec4<f32>(ndc, 0.75, 1.0);
    let a = a4.xyz / a4.w;
    let b = b4.xyz / b4.w;
    let fwd = camera.clusters.view_z;
    var dir = b - a;
    let along = dot(fwd.xyz, dir);
    dir = dir / select(along, 1e-6, abs(along) < 1e-6);
    let origin = a - dir * (dot(fwd.xyz, a) + fwd.w);
    return array<vec3<f32>, 2>(origin, dir);
}

// Faded to 1 over the last 10% as `calculate_shadow` does, so shadowed air never ends
// at a plane that moves with the camera.
fn sun_lit(world: vec3<f32>) -> f32 {
    let count = i32(shadow.params.x);
    let depth = dot(world - shadow.view_pos.xyz, shadow.view_dir.xyz);
    if (count == 0 || depth >= shadow.params.y) {
        return 1.0;
    }
    var c = 0;
    while (c < count - 1 && depth > shadow.splits[c]) {
        c = c + 1;
    }
    let p = shadow.light_space[c] * vec4<f32>(world, 1.0);
    let q = p.xyz / p.w;
    let uv = vec2<f32>(q.x * 0.5 + 0.5, -q.y * 0.5 + 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || q.z > 1.0) {
        return 1.0;
    }
    let bias = shadow.texels[c] / max(shadow.depth_ranges[c], 1e-6);
    let lit = textureSampleCompareLevel(t_shadow, s_shadow, uv, c, q.z - bias);
    let distance = shadow.params.y;
    let fade = clamp((depth - distance * 0.9) / (distance * 0.1), 0.0, 1.0);
    return mix(lit, 1.0, fade);
}

// A froxel is a volume: a sample landing on the lamp itself would otherwise spike that
// froxel from frame to frame.
fn lamps(light: LocalLight, world: vec3<f32>, to_eye: vec3<f32>, g: f32, near: f32) -> vec3<f32> {
    let travel = normalize(world - light.position);
    return local_light_radiance(light, world, near) * phase(g, dot(travel, to_eye));
}

fn ambient(world: vec3<f32>) -> vec3<f32> {
    if (params.probes_min.w < 0.5) {
        return ambient_average(lighting.ambient);
    }
    let uvw = (world - params.probes_min.xyz) / params.probes_size.xyz;
    return textureSampleLevel(t_probes, s_history, uvw, 0.0).rgb;
}

fn in_scatter(world: vec3<f32>, cluster: u32, near: f32) -> vec3<f32> {
    let g = params.medium.w;
    let to_eye = normalize(camera.camera_pos - world);
    var light = ambient(world) * params.albedo.w;
    for (var i = 0u; i < lighting.num_dir_lights; i = i + 1u) {
        let sun = lighting.dir_lights[i];
        if (sun.baked > 0.5) {
            continue;
        }
        let lit = select(1.0, sun_lit(world), i == 0u);
        light += sun.color * sun.intensity * lit * phase(g, dot(sun.direction, to_eye));
    }
    let live = cluster_ranges[cluster];
    for (var i = 0u; i < live.y; i = i + 1u) {
        let l = local_lights[cluster_lights[live.x + i]];
        var lit = 1.0;
        let tile = local_shadow_tile(l, world);
        if (tile != NO_SHADOW) {
            lit = sample_local_shadow(shadow_tiles[tile], t_shadow_atlas, s_shadow, l.position, world, vec3<f32>(0.0));
        }
        light += lamps(l, world, to_eye, g, near) * lit;
    }
    let baked = baked_ranges[cluster];
    for (var i = 0u; i < baked.y; i = i + 1u) {
        light += lamps(baked_lights[baked_indices[baked.x + i]], world, to_eye, g, near);
    }
    return light;
}

@compute @workgroup_size(4, 4, 4)
fn cs_scatter(@builtin(global_invocation_id) id: vec3<u32>) {
    let grid = params.grid.xyz;
    if (any(id >= grid)) {
        return;
    }
    let vol = camera.volumetric;
    let cell = vec3<f32>(id) + params.jitter.xyz;
    let uv = cell.xy / vec2<f32>(grid.xy);
    let ray = view_ray(uv);
    let world = ray[0] + ray[1] * froxel_depth(vol, cell.z);
    let h = max(world.y - params.medium.y, 0.0);
    let extinction = params.medium.x * exp(-h * params.medium.z);
    let cluster = cluster_index(camera.clusters, camera.view_proj, world);
    let near = 0.5 * (froxel_depth(vol, f32(id.z + 1u)) - froxel_depth(vol, f32(id.z)));
    var current = vec4<f32>(params.albedo.rgb * extinction * in_scatter(world, cluster, near), extinction);
    if (!finite4(current)) {
        current = vec4<f32>(0.0, 0.0, 0.0, extinction);
    }
    // A thick medium at a bright lamp's core would store Inf, and the integration's
    // `Inf - Inf` would leave a dark hole.
    current = vec4<f32>(min(current.rgb, vec3<f32>(MAX_FROXEL)), current.a);
    var out = current;
    if (params.jitter.w < 1.0) {
        let clip = params.prev_view_proj * vec4<f32>(world, 1.0);
        let ndc = clip.xy / max(clip.w, 1e-6);
        let prev_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
        let prev_depth = dot(params.prev_view_z.xyz, world) + params.prev_view_z.w;
        let prev = params.prev_volumetric;
        let w = froxel_slice(prev, prev_depth) / prev.y;
        let inside = clip.w > 0.0 && all(prev_uv >= vec2<f32>(0.0)) && all(prev_uv <= vec2<f32>(1.0)) && w <= 1.0;
        let history = textureSampleLevel(t_history, s_history, vec3<f32>(prev_uv, w), 0.0);
        if (inside && finite4(history)) {
            out = mix(history, current, params.jitter.w);
        }
    }
    textureStore(out_scatter, id, out);
}

@group(1) @binding(0) var t_scatter: texture_3d<f32>;
@group(1) @binding(1) var out_volume: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_integrate(@builtin(global_invocation_id) id: vec3<u32>) {
    let grid = params.grid.xyz;
    if (any(id.xy >= grid.xy)) {
        return;
    }
    let vol = camera.volumetric;
    let uv = (vec2<f32>(id.xy) + 0.5) / vec2<f32>(grid.xy);
    let stretch = length(view_ray(uv)[1]);
    var scattered = vec3<f32>(0.0);
    var through = 1.0;
    var near = 0.0;
    for (var z = 0u; z < grid.z; z = z + 1u) {
        let far = froxel_depth(vol, f32(z + 1u));
        let step = (far - near) * stretch;
        near = far;
        let v = textureLoad(t_scatter, vec3<u32>(id.xy, z), 0);
        let sigma = max(v.a, 1e-7);
        let fade = exp(-sigma * step);
        // Frostbite's integral of the slice's scattering over its length, so a thick
        // slice neither loses nor invents light.
        var slice = (v.rgb - v.rgb * fade) / sigma;
        if (!finite3(slice)) {
            slice = vec3<f32>(0.0);
        }
        scattered = min(scattered + through * slice, vec3<f32>(MAX_FROXEL));
        through *= fade;
        textureStore(out_volume, vec3<u32>(id.xy, z), vec4<f32>(scattered, through));
    }
}
