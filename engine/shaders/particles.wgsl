
#import lighting::ambient::ambient_average
#import common::{CameraUniforms, Fog, LightingUniforms, LocalLight, cluster_index, fog_factor, local_light_radiance}
#import fog::volumetric::{through_volume, volumetric_fog}
#import reactive
#import particles::gpu::{GpuParticle, EMPTY, gpu_emitter, life_t, lut_at}

struct ParticleGlobals {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_right: vec4<f32>,
    cam_up: vec4<f32>,
    cam_pos: vec4<f32>,
    cam_fwd: vec4<f32>,
    fog: Fog,
};

@group(3) @binding(0) var<uniform> globals: ParticleGlobals;
@group(1) @binding(0) var sprite_tex: texture_2d<f32>;
@group(1) @binding(1) var sprite_sampler: sampler;
@group(2) @binding(0) var scene_depth: texture_depth_2d;
@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var<uniform> lighting: LightingUniforms;
@group(0) @binding(7) var<storage, read> local_lights: array<LocalLight>;
@group(0) @binding(8) var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(0) @binding(9) var<storage, read> cluster_lights: array<u32>;
@group(3) @binding(2) var<storage, read> gpu_particles: array<GpuParticle>;
@group(0) @binding(17) var t_fog_volume: texture_3d<f32>;
@group(0) @binding(18) var s_linear_clamp: sampler;

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
    // 0 billboard, 1 along velocity, 2 flat in XZ, 3 upright.
    @location(7) mode: u32,
    @location(8) reactive: f32,
};

struct VsOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) fog: f32,
    @location(3) world: vec3<f32>,
    @location(4) soft: f32,
    // The volumetric fog: rgb the air's light, a what gets through; (0, 0, 0, 1) where the
    // camera reads no volume.
    @location(5) air: vec4<f32>,
    @location(6) @interpolate(flat) reactive: f32,
};

// A unit quad's corner from the vertex index (indices 0,1,2,0,2,3), by arithmetic: naga
// forbids indexing a const array by a non-constant.
fn corner_of(vid: u32) -> vec2<f32> {
    let x = select(-0.5, 0.5, vid == 1u || vid == 2u);
    let y = select(-0.5, 0.5, vid == 2u || vid == 3u);
    return vec2<f32>(x, y);
}

fn unit_or(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    return select(fallback, v / len, len > 1e-5);
}

// World-space offset; `corner` is in [-0.5, 0.5]².
fn corner_offset(inst: InstanceInput, corner: vec2<f32>) -> vec3<f32> {
    let cam_right = globals.cam_right.xyz;
    let cam_up = globals.cam_up.xyz;
    let to_particle = inst.center - globals.cam_pos.xyz;
    if (inst.mode == 1u) {
        let axis = inst.stretch.xyz;
        if (dot(axis, axis) > 0.5) {
            let side = unit_or(cross(to_particle, axis), cam_right);
            return side * (corner.x * inst.size) + axis * (corner.y * inst.stretch.w);
        }
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

fn particle_light(world: vec3<f32>, light: vec4<f32>, near: f32) -> vec3<f32> {
    var total = ambient_average(lighting.ambient);
    if (light.w > 1.5) {
        total = light.rgb;
    }
    // No normal: each light counts as on a matte surface turned toward it (radiance / π).
    let inv_pi = 0.31830988;
    for (var i = 0u; i < lighting.num_dir_lights; i = i + 1u) {
        total += lighting.dir_lights[i].color * lighting.dir_lights[i].intensity * inv_pi;
    }
    let range = cluster_ranges[cluster_index(camera.clusters, camera.view_proj, world)];
    for (var i = 0u; i < range.y; i = i + 1u) {
        total += local_light_radiance(local_lights[cluster_lights[range.x + i]], world, near) * inv_pi;
    }
    return total;
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32, inst: InstanceInput) -> VsOut {
    return expand(inst, vid);
}

fn gpu_frame(p: GpuParticle, sheet: vec4<f32>) -> f32 {
    let frames = u32(sheet.x * sheet.y);
    if (frames <= 1u) {
        return 0.0;
    }
    let t = min(life_t(p), 0.99999);
    let played = u32(floor(t * sheet.z * f32(frames)));
    return f32((played + p.start_frame) % frames);
}

@vertex
fn vs_gpu(@builtin(vertex_index) vid: u32, @builtin(instance_index) slot: u32) -> VsOut {
    let p = gpu_particles[slot];
    if (p.next == EMPTY) {
        var out: VsOut;
        out.clip_position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return out;
    }
    let e = gpu_emitter;
    let t = life_t(p);
    let size = max(p.size * lut_at(0u, t).x, 0.0);
    let velocity = (e.frame * vec4<f32>(p.velocity, 0.0)).xyz;
    let speed = length(velocity);
    var inst: InstanceInput;
    inst.center = (e.frame * vec4<f32>(p.position, 1.0)).xyz;
    inst.size = size;
    inst.rotation = p.rotation;
    inst.color = p.color * lut_at(1u, t);
    inst.stretch = vec4<f32>(select(vec3<f32>(0.0), velocity / speed, speed > 0.0), size * e.stretch.x + speed * e.stretch.y);
    inst.light = e.light;
    inst.sheet = vec4<f32>(e.sheet.x, e.sheet.y, gpu_frame(p, e.sheet), e.sheet.w);
    inst.mode = e.info.x;
    inst.reactive = e.stretch.z;
    return expand(inst, vid);
}

fn expand(inst: InstanceInput, vid: u32) -> VsOut {
    let corner = corner_of(vid);
    // UV: x follows the +0.5 side, y is flipped so the sprite is upright.
    let uv = vec2<f32>(corner.x + 0.5, 0.5 - corner.y);
    let world = inst.center + corner_offset(inst, corner);

    var color = inst.color;
    if (inst.light.w > 0.5) {
        // Half the sprite's size floors the lamp distance, so a grazed corner gets no spike.
        color = vec4<f32>(color.rgb * particle_light(world, inst.light, inst.size * 0.5), color.a);
    }

    var out: VsOut;
    out.clip_position = globals.view_proj * vec4<f32>(world, 1.0);
    out.uv = sheet_uv(uv, inst.sheet);
    out.color = color;
    out.fog = fog_factor(globals.fog, world, globals.cam_pos.xyz);
    out.world = world;
    out.soft = inst.sheet.w;
    out.reactive = inst.reactive;
    out.air = volumetric_fog(t_fog_volume, s_linear_clamp, camera.volumetric, camera.view_proj, camera.clusters.view_z, world);
    return out;
}

// Soft particles: 0 where the sprite touches the scene behind it, 1 once it is `in.soft` world
// units in front of it (measured along the camera's forward axis).
fn soft_fade(in: VsOut) -> f32 {
    if (in.soft <= 0.0) {
        return 1.0;
    }
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

@fragment
fn fs_alpha(in: VsOut) -> @location(0) vec4<f32> {
    return alpha_color(in);
}

fn alpha_color(in: VsOut) -> vec4<f32> {
    let c = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    return vec4<f32>(through_volume(mix(c.rgb, globals.fog.color, in.fog), in.air), c.a * soft_fade(in));
}

// Additive sparks fade OUT instead: adding the fog colour would brighten the fog itself, so
// fogged light simply contributes less.
@fragment
fn fs_additive(in: VsOut) -> @location(0) vec4<f32> {
    return additive_color(in);
}

fn additive_color(in: VsOut) -> vec4<f32> {
    let c = textureSample(sprite_tex, sprite_sampler, in.uv) * in.color;
    return vec4<f32>(c.rgb * (1.0 - in.fog) * in.air.a, c.a * soft_fade(in));
}

struct ReactiveOut {
    @location(0) color: vec4<f32>,
    @location(1) reactive: vec4<f32>,
};

@fragment
fn fs_alpha_reactive(in: VsOut) -> ReactiveOut {
    let c = alpha_color(in);
    return ReactiveOut(c, reactive::texel(reactive::over(c.a), in.reactive, camera.stack.x));
}

@fragment
fn fs_additive_reactive(in: VsOut) -> ReactiveOut {
    let c = additive_color(in);
    return ReactiveOut(c, reactive::texel(reactive::added(c.rgb * c.a), in.reactive, camera.stack.x));
}
