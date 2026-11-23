// Baked shaders may import any name: none moves out.
#import fog::height::{fog_height_mean, fog_thickness}
#import lighting::falloff::{light_falloff_near, spot_cone}
#import lighting::reflection_probe::ReflectionProbe
#import sky::noise::{cloud_shift, sky_fbm}
#import sky::look::{SKY_HORIZON_TO_WHITE, SKY_GROUND_OF_ZENITH, SKY_ZENITH_EXPONENT, SKY_GROUND_EXPONENT, CLOUD_DOME_MIN_HEIGHT, CLOUD_CELLS_PER_UNIT, CLOUD_FBM_EDGE_START, CLOUD_FBM_EDGE_END, CLOUD_BAND_START, CLOUD_BAND_END, CLOUD_TO_WHITE}

// Mirrors `FogUniform`. `mode`: 0 off, 1 linear, 2 exponential, 3 exponential².
struct Fog {
    color: vec3<f32>,
    mode: u32,
    start: f32,
    end: f32,
    density: f32,
    height_falloff: f32,
    base_height: f32,
    _pad_a: f32,
    _pad_b: f32,
    _pad_c: f32,
};

// Mirrors `ClusterUniform`.
struct Clusters {
    view_z: vec4<f32>,
    // x scale, y bias, z 1 log2 (perspective) / 0 linear slices, w near plane.
    slices: vec4<f32>,
    dims: vec4<u32>,
};

// Mirrors `CameraUniform`.
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    // Scaled game seconds: frozen while paused, 0 in edit mode.
    time: f32,
    fog: Fog,
    clusters: Clusters,
    prev_view_proj: mat4x4<f32>,
    stack: vec4<f32>,
    inv_view_proj: mat4x4<f32>,
    // x steps, y step length, z minimum smoothness, w fade-in smoothness; zero
    // without SSR.
    ssr: vec4<f32>,
    ssil: vec4<f32>,
    // x: log2(scene/output) under TAAU, else 0.
    lod: vec4<f32>,
    volumetric: vec4<f32>,
};

// Mirrors `AmbientLightUniform`.
struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
    gradient_sky: vec3<f32>,
    // 0 `color`'s hemisphere, 1 trilight, 2 the sky SH.
    mode: f32,
    gradient_equator: vec3<f32>,
    _pad_equator: f32,
    gradient_ground: vec3<f32>,
    _pad_ground: f32,
    // Intensity applied, convolved over π.
    sky_sh: array<vec4<f32>, 9>,
};

struct DirectionalLight {
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    baked: f32,
    bounce_baked: f32,
    // 0 to 3; -1 for none.
    mask_channel: f32,
};

// Mirrors `LightingUniform`.
struct LightingUniforms {
    ambient: AmbientLight,
    // `sky_gradient.wgsl` reads `ambient`, `sky` as a prefix.
    sky: ProceduralSky,
    dir_lights: array<DirectionalLight, 4>,
    num_dir_lights: u32,
    sky_textured: f32,
    env_reflection: f32,
    diffuse_wrap: f32,
    baked_specular: f32,
    shadowmask: f32,
    masks_bound: f32,
    // Scalars: a vec3 would align to 16 and move `refl_probes`.
    baked_specular_pad_c: f32,
    refl_probes: array<ReflectionProbe, 32>,
};

// Mirrors `lights::LocalLight`.
struct LocalLight {
    position: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
    // `clusters::lights::KIND_*`; cones are half-angle cosines.
    kind: u32,
    inner_cone: f32,
    outer_cone: f32,
    shadow: u32,
    baked: f32,
    shadow_fade: f32,
    near: f32,
    bounce_baked: f32,
    mask_channel: f32,
};

// Mirrors `ClusterGrid::cluster_at`.
fn cluster_index(c: Clusters, view_proj: mat4x4<f32>, world: vec3<f32>) -> u32 {
    let clip = view_proj * vec4<f32>(world, 1.0);
    let ndc = clip.xy / max(clip.w, 1e-6);
    let tiles = vec2<f32>(c.dims.xy);
    let tile = vec2<u32>(clamp(floor((ndc * 0.5 + 0.5) * tiles), vec2<f32>(0.0), tiles - 1.0));
    let depth = max(dot(c.view_z.xyz, world) + c.view_z.w, c.slices.w);
    let f = select(depth, log2(depth), c.slices.z > 0.5);
    let slice = u32(clamp(floor(f * c.slices.x + c.slices.y), 0.0, f32(c.dims.z - 1u)));
    return tile.x + c.dims.x * (tile.y + c.dims.y * slice);
}

fn local_light_radiance(light: LocalLight, world: vec3<f32>, near: f32) -> vec3<f32> {
    let d = distance(light.position, world);
    if (d >= light.range) {
        return vec3<f32>(0.0);
    }
    var cone = 1.0;
    if (light.kind != 0u) {
        let theta = dot((light.position - world) / max(d, 1e-6), -light.direction);
        cone = select(spot_cone(theta, light.inner_cone, light.outer_cone), max(theta, 0.0), light.kind == 2u);
    }
    return light.color * light.intensity * cone * light_falloff_near(d, light.range, max(near, light.near));
}

struct Decal {
    world_to_decal: mat4x4<f32>,
    right: vec4<f32>,
    up: vec4<f32>,
    forward: vec4<f32>,
    color: vec4<f32>,
    surface: vec4<f32>,
    fade: vec4<f32>,
    layers: vec4<u32>,
};

const NO_LAYER: u32 = 0xffffffffu;

struct DecalSurface {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
};

// `dx`/`dy` come from uniform control flow.
fn apply_decal(
    s: DecalSurface, d: Decal, world: vec3<f32>, facing: vec3<f32>,
    dx: vec3<f32>, dy: vec3<f32>,
    t_color: texture_2d_array<f32>, t_data: texture_2d_array<f32>, samp: sampler,
) -> DecalSurface {
    let local = (d.world_to_decal * vec4<f32>(world, 1.0)).xyz;
    if (any(abs(local) > vec3<f32>(0.5))) {
        return s;
    }
    let angle = clamp((dot(facing, d.forward.xyz) - d.fade.y) / max(d.forward.w - d.fade.y, 1e-4), 0.0, 1.0);
    let caps = 1.0 - smoothstep(0.35, 0.5, abs(local.z));
    let uv = vec2<f32>(local.x + 0.5, 0.5 - local.y);
    let to_decal = mat3x3<f32>(d.world_to_decal[0].xyz, d.world_to_decal[1].xyz, d.world_to_decal[2].xyz);
    let gx = to_decal * dx;
    let gy = to_decal * dy;
    let uv_dx = vec2<f32>(gx.x, -gx.y);
    let uv_dy = vec2<f32>(gy.x, -gy.y);

    var texel = vec4<f32>(1.0);
    if (d.layers.x != NO_LAYER) {
        texel = textureSampleGrad(t_color, samp, uv, i32(d.layers.x), uv_dx, uv_dy);
    }
    let coverage = texel.a * d.color.a * angle * caps;
    if (coverage <= 0.001) {
        return s;
    }
    var out = s;
    out.albedo = mix(s.albedo, texel.rgb * d.color.rgb, coverage * d.right.w);
    if (d.layers.y != NO_LAYER) {
        let n = textureSampleGrad(t_data, samp, uv, i32(d.layers.y), uv_dx, uv_dy).xyz * 2.0 - 1.0;
        let T = normalize(d.right.xyz - s.normal * dot(d.right.xyz, s.normal));
        let B = cross(s.normal, T);
        let bent = normalize(T * n.x + B * n.y + s.normal * n.z);
        out.normal = normalize(mix(s.normal, bent, coverage * d.up.w));
    }
    var metallic = d.surface.x;
    if (d.layers.z != NO_LAYER) {
        metallic *= textureSampleGrad(t_data, samp, uv, i32(d.layers.z), uv_dx, uv_dy).b;
    }
    var roughness = d.surface.y;
    if (d.layers.w != NO_LAYER) {
        roughness *= textureSampleGrad(t_data, samp, uv, i32(d.layers.w), uv_dx, uv_dy).g;
    }
    out.metallic = mix(s.metallic, metallic, coverage * d.surface.z);
    out.roughness = clamp(mix(s.roughness, roughness, coverage * d.surface.w), 0.04, 1.0);
    out.occlusion = s.occlusion * mix(1.0, d.fade.x, coverage);
    return out;
}

const NO_SHADOW: u32 = 0xffffffffu;

// `params.x`: one texel's world size a metre from the light.
struct ShadowTile {
    view_proj: mat4x4<f32>,
    rect: vec4<f32>,
    params: vec4<f32>,
};

fn local_shadow_tile(light: LocalLight, world: vec3<f32>) -> u32 {
    if (light.shadow == NO_SHADOW || light.kind != 0u) {
        return light.shadow;
    }
    let d = world - light.position;
    let a = abs(d);
    var face = select(5u, 4u, d.z > 0.0);
    if (a.x >= a.y && a.x >= a.z) {
        face = select(1u, 0u, d.x > 0.0);
    } else if (a.y >= a.z) {
        face = select(3u, 2u, d.y > 0.0);
    }
    return light.shadow + face;
}

// Pushed 0.5 to 2 texels' world size along N, more grazing; taps stay 1.5 texels inside the
// tile.
fn sample_local_shadow(
    tile: ShadowTile,
    atlas: texture_depth_2d,
    s: sampler_comparison,
    light_pos: vec3<f32>,
    world: vec3<f32>,
    N: vec3<f32>,
) -> f32 {
    let to_light = light_pos - world;
    let dist = length(to_light);
    let NdotL = clamp(dot(N, to_light / max(dist, 1e-6)), 0.0, 1.0);
    let offset = N * dist * tile.params.x * (0.5 + 1.5 * (1.0 - NdotL));
    let clip = tile.view_proj * vec4<f32>(world + offset, 1.0);
    if (clip.w <= 0.0) {
        return 1.0;
    }
    let ndc = clip.xyz / clip.w;
    if (any(abs(ndc.xy) > vec2<f32>(1.0)) || ndc.z > 1.0) {
        return 1.0;
    }
    let texel = 1.0 / f32(textureDimensions(atlas).x);
    let lo = tile.rect.xy + vec2<f32>(texel * 1.5);
    let hi = tile.rect.xy + tile.rect.zw - vec2<f32>(texel * 1.5);
    let uv = tile.rect.xy + vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * tile.rect.zw;
    var lit = 0.0;
    for (var x = -1; x <= 1; x = x + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            let tap = clamp(uv + vec2<f32>(f32(x), f32(y)) * texel, lo, hi);
            lit += textureSampleCompareLevel(atlas, s, tap, ndc.z);
        }
    }
    return lit / 9.0;
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
    @location(5) tangent: vec4<f32>,
    @location(6) lightmap_uv: vec2<f32>,
};

fn blend_joints(
    j0: mat4x4<f32>,
    j1: mat4x4<f32>,
    j2: mat4x4<f32>,
    j3: mat4x4<f32>,
    weights: vec4<f32>,
) -> mat4x4<f32> {
    if (weights.x + weights.y + weights.z + weights.w < 0.01) {
        return mat4x4<f32>(vec4<f32>(1.0, 0.0, 0.0, 0.0), vec4<f32>(0.0, 1.0, 0.0, 0.0), vec4<f32>(0.0, 0.0, 1.0, 0.0), vec4<f32>(0.0, 0.0, 0.0, 1.0));
    }
    return j0 * weights.x + j1 * weights.y + j2 * weights.z + j3 * weights.w;
}

// Metres, past any level's far wall: the sky fogs no less than the farthest geometry.
const FOG_SKY_DISTANCE: f32 = 1000.0;

fn fog_factor(fog: Fog, world_pos: vec3<f32>, eye: vec3<f32>) -> f32 {
    return fog_over(fog, distance(world_pos, eye), eye.y, world_pos.y);
}

fn fog_over(fog: Fog, length: f32, from_y: f32, to_y: f32) -> f32 {
    let h = fog_height_mean(fog.height_falloff, from_y - fog.base_height, to_y - fog.base_height);
    return fog_thickness(fog.mode, vec3<f32>(fog.start, fog.end, fog.density), length, h);
}

fn sky_fog(fog: Fog, color: vec3<f32>, dir: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let f = fog_factor(fog, eye + dir * FOG_SKY_DISTANCE, eye);
    let horizon = 1.0 - smoothstep(0.0, 0.4, dir.y);
    return mix(color, fog.color, f * horizon);
}

const PI: f32 = 3.14159265359;

// `y` is clamped: rounding can pass 1, a NaN in `acos`.
fn equirect_uv(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta = acos(clamp(dir.y, -1.0, 1.0));
    return vec2<f32>((phi + PI) / (2.0 * PI), theta / PI);
}

struct ProceduralSky {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    ground: vec4<f32>,
    clouds: vec4<f32>,
};

// `procedural_sky` stays affine in `ambient` for a fixed `sky` (`a * ambient + b`, `a` a
// scalar per direction): `sky/prefilter.wgsl` bakes both terms once.

fn procedural_sky(sky: ProceduralSky, ambient: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let zenith = select(ambient, sky.zenith.rgb, sky.zenith.w > 0.5);
    let horizon_color = select(mix(zenith, vec3<f32>(1.0), SKY_HORIZON_TO_WHITE), sky.horizon.rgb, sky.horizon.w > 0.5);
    let ground_color = select(zenith * SKY_GROUND_OF_ZENITH, sky.ground.rgb, sky.ground.w > 0.5);

    let up = clamp(dir.y, 0.0, 1.0);
    let down = clamp(-dir.y, 0.0, 1.0);
    var color = mix(horizon_color, zenith, pow(up, SKY_ZENITH_EXPONENT));
    color = mix(color, ground_color, pow(down, SKY_GROUND_EXPONENT));

    if (dir.y > 0.0) {
        let dome = dir.xz / max(dir.y, CLOUD_DOME_MIN_HEIGHT);
        let fbm = sky_fbm(dome * CLOUD_CELLS_PER_UNIT) + cloud_shift(sky.clouds.x);
        let coverage = smoothstep(CLOUD_FBM_EDGE_START, CLOUD_FBM_EDGE_END, fbm);
        let band = smoothstep(CLOUD_BAND_START, CLOUD_BAND_END, up);
        let cloud_color = mix(zenith, vec3<f32>(1.0), CLOUD_TO_WHITE);
        color = mix(color, cloud_color, coverage * band * sky.clouds.y);
    }
    return color;
}
