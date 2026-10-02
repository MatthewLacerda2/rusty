// assets/shaders/particles.wgsl — the sprite particle pass (#13, #440).
//
// Each particle is one instance; the vertex shader expands it into a quad whose
// orientation depends on its render mode:
//   0 billboard  — faces the camera, spun by `rotation` about the view axis;
//   1 stretched  — long axis along the particle's velocity, turned to face the camera
//                  as far as that axis allows (sparks, tracers);
//   2 horizontal — flat in the world XZ plane, spun about Y (splashes, rings);
//   3 vertical   — faces the camera but stays upright, spun in its plane.
// (Mesh particles never reach this pass — they draw through the forward path.)
//
// A flipbook picks one cell of a `columns × rows` sprite sheet per particle. Lit
// particles are shaded per vertex against the same lighting uniform the forward
// pass reads (normal-free: a smoke puff is as bright as a matte surface turned
// toward each light). Soft particles fade out as they near the scene depth behind
// them. Every mode fogs per vertex (#437); alpha/additive blending is selected by
// the pipeline, which picks the matching fragment entry point.

#import common::{CameraUniforms, Fog, LightingUniforms, LocalLight, cluster_index, fog_factor, local_light_radiance}

struct ParticleGlobals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,   // xyz used
    cam_up: vec4<f32>,      // xyz used
    cam_pos: vec4<f32>,     // xyz used
    cam_fwd: vec4<f32>,     // xyz used
    fog: Fog,
};

@group(0) @binding(0) var<uniform> globals: ParticleGlobals;
@group(1) @binding(0) var sprite_tex: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;
// The scene depth (read-only this pass) — soft particles fade against it.
@group(2) @binding(0) var scene_depth: texture_depth_2d;
// The renderer's group 0 (camera + lighting + env + light clusters, #434); the
// camera is read only to find a vertex's light cluster.
@group(3) @binding(0) var<uniform> camera: CameraUniforms;
@group(3) @binding(1) var<uniform> lighting: LightingUniforms;
@group(3) @binding(7) var<storage, read> local_lights: array<LocalLight>;
@group(3) @binding(8) var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(3) @binding(9) var<storage, read> cluster_lights: array<u32>;

struct InstanceInput {
    @location(0) center: vec3<f32>,
    @location(1) size: f32,
    @location(2) rotation: f32,
    @location(3) color: vec4<f32>,
    // Stretched: unit velocity direction (xyz) and quad length (w).
    @location(4) stretch: vec4<f32>,
    // Lit: rgb = probe ambient (when w == 2); w = 0 unlit, 1 flat ambient, 2 probe.
    @location(5) light: vec4<f32>,
    // Flipbook columns, rows and frame; soft-fade distance (0 = off).
    @location(6) sheet: vec4<f32>,
    @location(7) mode: u32,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    // Scene fog at this corner — per vertex, since a particle is small.
    @location(2) fog: f32,
    @location(3) world: vec3<f32>,
    @location(4) soft: f32,
};

// Corner of a unit quad (two triangles via the index buffer 0,1,2,0,2,3) derived
// arithmetically from the vertex index so no dynamically-indexed array is needed
// (naga forbids indexing a const array by a non-constant). Corners:
//   0:(-0.5,-0.5) 1:(0.5,-0.5) 2:(0.5,0.5) 3:(-0.5,0.5)
// Winding is irrelevant — the pipeline disables culling (cull_mode: None).
fn corner_of(vid: u32) -> vec2<f32> {
    let x = select(-0.5, 0.5, vid == 1u || vid == 2u);
    let y = select(-0.5, 0.5, vid == 2u || vid == 3u);
    return vec2<f32>(x, y);
}

// `v` normalised, or `fallback` when it is (near) zero.
fn unit_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    return select(fallback, v / len, len > 1e-5);
}

// The quad's world-space corner offset for `corner` (in [-0.5, 0.5]²).
fn corner_offset(inst: InstanceInput, corner: vec2<f32>) -> vec3<f32> {
    let cam_right = globals.cam_right.xyz;
    let cam_up = globals.cam_up.xyz;
    let to_particle = inst.center - globals.cam_pos.xyz;
    if (inst.mode == 1u) {
        let axis = inst.stretch.xyz;
        if (dot(axis, axis) > 0.5) {
            // Width across the axis, turned toward the camera.
            let side = unit_or(cross(to_particle, axis), cam_right);
            return side * (corner.x * inst.size) + axis * (corner.y * inst.stretch.w);
        }
        // At rest a stretched particle has no axis: draw it as a billboard.
    }
    var right = cam_right;
    var up = cam_up;
    if (inst.mode == 2u) {
        right = vec3<f32>(1.0, 0.0, 0.0);
        up = vec3<f32>(0.0, 0.0, -1.0);
    } else if (inst.mode == 3u) {
        up = vec3<f32>(0.0, 1.0, 0.0);
        right = unit_or(cross(to_particle, up), cam_right);
    }
    // Spin the corner in the quad's plane (UVs stay with the corner).
    let c = cos(inst.rotation);
    let s = sin(inst.rotation);
    let spun = vec2<f32>(corner.x * c - corner.y * s, corner.x * s + corner.y * c);
    return right * (spun.x * inst.size) + up * (spun.y * inst.size);
}

// The flipbook cell `uv` (in [0, 1]² of one frame) maps to on the sheet.
fn sheet_uv(uv: vec2<f32>, sheet: vec4<f32>) -> vec2<f32> {
    let cols = max(sheet.x, 1.0);
    let rows = max(sheet.y, 1.0);
    let col = sheet.z - cols * floor(sheet.z / cols);
    let row = floor(sheet.z / cols);
    return (vec2<f32>(col, row) + uv) / vec2<f32>(cols, rows);
}

// Light arriving at a point from the scene lights, with no surface normal: each
// light counts as it would on a matte surface turned toward it (radiance / pi).
// Point + spot falloff is the forward pass's (`local_light_radiance`).
fn particle_light(world: vec3<f32>, light: vec4<f32>) -> vec3<f32> {
    // Flat ambient: the forward pass's sky/ground gradient averaged over directions.
    var total = lighting.ambient.color * lighting.ambient.intensity * 0.625;
    if (light.w > 1.5) {
        total = light.rgb;
    }
    let inv_pi = 0.31830988;
    for (var i = 0u; i < lighting.num_dir_lights; i = i + 1u) {
        total += lighting.dir_lights[i].color * lighting.dir_lights[i].intensity * inv_pi;
    }
    // Point and spot lights through this vertex's cluster (#434).
    let range = cluster_ranges[cluster_index(camera.clusters, camera.view_proj, world)];
    for (var i = 0u; i < range.y; i = i + 1u) {
        total += local_light_radiance(local_lights[cluster_lights[range.x + i]], world) * inv_pi;
    }
    return total;
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceInput) -> VsOut {
    let corner = corner_of(vid);
    // UV: x follows the +0.5 side, y is flipped so the sprite is upright.
    let uv = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    let world = inst.center + corner_offset(inst, corner);

    var color = inst.color;
    if (inst.light.w > 0.5) {
        color = vec4<f32>(color.rgb * particle_light(world, inst.light), color.a);
    }

    var out: VsOut;
    out.clip_position = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = sheet_uv(uv, inst.sheet);
    out.color = color;
    out.fog = fog_factor(globals.fog, world, globals.cam_pos.xyz);
    out.world = world;
    out.soft = inst.sheet.w;
    return out;
}

// Soft particles: 0 where the sprite touches the scene behind it, 1 once it is
// `in.soft` world units in front of it (measured along the camera's forward axis).
fn soft_fade(in: VsOut) -> f32 {
    if (in.soft <= 0.0) {
        return 1.0;
    }
    // The depth target is the colour target's size, so the fragment's pixel indexes it.
    let size = vec2<f32>(textureDimensions(scene_depth));
    let pixel = clamp(vec2<i32>(in.clip_position.xy), vec2<i32>(0), vec2<i32>(size) - 1);
    let depth = textureLoad(scene_depth, pixel, 0);
    let ndc = vec2<f32>(
        in.clip_position.x / size.x * 2.0 - 1.0,
        1.0 - in.clip_position.y / size.y * 2.0,
    );
    let h = globals.inv_view_proj * vec4<f32>(ndc, depth, 1.0);
    let scene_pos = h.xyz / h.w;
    let fwd = globals.cam_fwd.xyz;
    let gap = dot(scene_pos - in.world, fwd);
    return clamp(gap / in.soft, 0.0, 1.0);
}

// Alpha-blended smoke fades into the fog colour, like the surface behind it.
@fragment
fn fs_alpha(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    return vec4<f32>(mix(c.rgb, globals.fog.color, in.fog), c.a * soft_fade(in));
}

// Additive sparks fade OUT instead: adding the fog colour would brighten the fog
// itself, so fogged light simply contributes less.
@fragment
fn fs_additive(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    return vec4<f32>(c.rgb * (1.0 - in.fog), c.a * soft_fade(in));
}
