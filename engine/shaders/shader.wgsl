#import common::{CameraUniforms, Decal, DecalSurface, LightingUniforms, LocalLight, NO_SHADOW, ShadowTile, VertexInput, apply_decal, apply_fog, blend_joints, cluster_index, local_light_radiance, local_shadow_tile, procedural_sky, sample_local_shadow}

struct EntityUniforms {
    model_matrix: mat4x4<f32>,
    color_tint: vec4<f32>,
    use_texture: u32,
    is_lit: u32,
    metallic: f32,
    roughness: f32,
    // Mirrors `EntityUniform` (src/render/mod.rs) byte-for-byte (#202, #207): map
    // flags. metallic/roughness scale their scalar by the sampled channel; normal
    // perturbs the shading normal; emissive modulates the emissive factor. Four u32s
    // fill the run so the `vec4` that follows is 16-byte aligned.
    use_metallic_map: u32,
    use_roughness_map: u32,
    use_normal_map: u32,
    use_emissive_map: u32,
    // Flat emissive factor (#222): rgb glow added after lighting in `fs_main`; the
    // 4th lane is unused. `vec4` to match the Rust `[f32; 4]` byte-for-byte.
    emissive: vec4<f32>,
    // Cutout alpha-test (#242): `use_cutout == 1` discards fragments whose final
    // alpha is below `alpha_cutoff`.
    use_cutout: u32,
    alpha_cutoff: f32,
    // Where this draw's joint 0 sits in `bones` (#455); 0 is the shared identity.
    // Mirrors `EntityUniform` (src/render/gpu/uniforms.rs) byte-for-byte.
    bone_base: u32,
    // 1 when this draw folds in its cluster's decals (#638).
    receive_decals: u32,
};

// One instance of an instanced draw (#470), indexed by `instance_index`. Mirrors
// `InstanceData` (src/render/gpu/uniforms.rs) byte-for-byte. Only per-copy data lives
// here — the world matrix and the light-probe SH (#240: when `use_sh == 1` the ambient
// term is reconstructed from `sh`, xyz = RGB radiance). Material flags stay in the
// per-draw `EntityUniforms`, so texture sampling never depends on an instance value.
struct InstanceData {
    model_matrix: mat4x4<f32>,
    use_sh: u32,
    // Baked lightmap (#438): atlas page + 1 (0 for none), and the scale/offset from
    // the mesh's lightmap UV into that page.
    lightmap_page: u32,
    // 1 when the lightmap's direction page is bound (#810).
    lightmap_directional: u32,
    _ipad2: u32,
    lightmap_st: vec4<f32>,
    sh: array<vec4<f32>, 9>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

@group(0) @binding(2)
var t_skybox: texture_2d<f32>;
@group(0) @binding(3)
var s_skybox: sampler;

// Active reflection probe's prefiltered cubemap (#245). Sampled with `textureSampleLevel`
// (roughness -> mip) and box-projected parallax when `refl_has_cubemap > 0.5`; otherwise a
// 1x1 black fallback is bound and ignored (the skybox is sampled instead).
@group(0) @binding(4)
var t_refl_cube: texture_cube<f32>;
@group(0) @binding(5)
var s_refl_cube: sampler;

// Clustered lights (#434): the frame's point/spot lights, each cluster's
// (offset, count) into `cluster_lights`, and that flat list of light indices.
// Binding 6 is the shadow module's (below).
@group(0) @binding(7)
var<storage, read> local_lights: array<LocalLight>;
@group(0) @binding(8)
var<storage, read> cluster_ranges: array<vec2<u32>>;
@group(0) @binding(9)
var<storage, read> cluster_lights: array<u32>;

// Surface decals (#638): the frame's decals, and the atlas their maps live in, read
// through its sRGB view (albedo) and its raw view (normal, metallic, roughness). A
// cluster's decals follow the light ranges in `cluster_ranges`, indexing
// `cluster_lights` like its lights do.
@group(0) @binding(10)
var<storage, read> decals: array<Decal>;
@group(0) @binding(11)
var t_decal_color: texture_2d_array<f32>;
@group(0) @binding(12)
var t_decal_data: texture_2d_array<f32>;
@group(0) @binding(13)
var s_decal: sampler;

// Baked lightmap atlas pages (#438): linear RGBM, one layer per page, read at the
// instance's page through its scale/offset; a 1x1 black page when none are baked.
@group(0) @binding(14)
var t_lightmaps: texture_2d_array<f32>;
@group(0) @binding(15)
var s_lightmaps: sampler;
// Their direction pages (#810), same layers and UVs: RGB the dominant incoming
// direction in [0,1], A its directionality; a 1x1 zero page when not baked.
@group(0) @binding(16)
var t_lightmap_dirs: texture_2d_array<f32>;

@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

// Every skinned draw's joint matrices back to back (#455), sized per skin — no joint
// cap. A draw reads its run from `entity.bone_base`; element 0 is the identity.
@group(1) @binding(1)
var<storage, read> bones: array<mat4x4<f32>>;

@group(1) @binding(2)
var<storage, read> instances: array<InstanceData>;

@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;
@group(2) @binding(2)
var t_metallic: texture_2d<f32>;
@group(2) @binding(3)
var t_roughness: texture_2d<f32>;
@group(2) @binding(4)
var t_normal: texture_2d<f32>;
@group(2) @binding(5)
var t_emissive: texture_2d<f32>;

// The sun's cascaded shadow maps (#435): one light volume per slice of the view,
// written by `render::passes::shadows::CascadeUniform`.
struct ShadowCascades {
    light_space: array<mat4x4<f32>, 4>,
    splits: vec4<f32>,       // view depth where each cascade ends
    texels: vec4<f32>,       // world size of one texel, per cascade
    depth_ranges: vec4<f32>, // world depth each cascade's [0, 1] spans
    view_pos: vec4<f32>,
    view_dir: vec4<f32>,
    params: vec4<f32>,       // x count, y shadow distance, z blend fraction
};

@group(3) @binding(0)
var<uniform> shadow: ShadowCascades;
@group(3) @binding(1)
var t_shadow: texture_depth_2d_array;
@group(3) @binding(2)
var s_shadow: sampler_comparison;
// Screen-space ambient occlusion (#436), one texel per pixel; a 1x1 white texture
// wherever the SSAO pass did not run (off, Low tier, the transparent pass).
@group(3) @binding(3)
var t_ao: texture_2d<f32>;
// The point/spot shadow atlas (#468) and its tiles; a light's `shadow` indexes them.
@group(3) @binding(4)
var t_shadow_atlas: texture_depth_2d;
@group(3) @binding(5)
var<storage, read> shadow_tiles: array<ShadowTile>;

// Fold the decals binned into `cluster` into `s`, oldest first (FIFO, #638): each
// newer decal lands over the older ones. Derivatives are taken here, before the
// cluster's own loop, so the walk may diverge per pixel.
fn fold_decals(s: DecalSurface, cluster: u32, world: vec3<f32>, facing: vec3<f32>) -> DecalSurface {
    let dx = dpdx(world);
    let dy = dpdy(world);
    let dims = camera.clusters.dims;
    let range = cluster_ranges[dims.x * dims.y * dims.z + cluster];
    var out = s;
    for (var i = 0u; i < range.y; i = i + 1u) {
        let d = decals[cluster_lights[range.x + i]];
        out = apply_decal(out, d, world, facing, dx, dy, t_decal_color, t_decal_data, s_decal);
    }
    return out;
}

fn ambient_occlusion(frag: vec2<f32>) -> f32 {
    let last = textureDimensions(t_ao) - vec2<u32>(1u);
    return textureLoad(t_ao, min(vec2<u32>(frag), last), 0).r;
}


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    // World-space tangent for normal mapping; `w` carries the handedness sign so the
    // fragment shader can reconstruct the bitangent as `w * cross(N, T)`.
    @location(3) world_tangent: vec4<f32>,
    // Which `instances` entry this fragment belongs to (#470), for its probe SH.
    @location(4) @interpolate(flat) instance: u32,
    // Where this fragment sits in its lightmap page (#438), already scaled/offset.
    @location(5) lightmap_uv: vec2<f32>,
};

@vertex
fn vs_main(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    var out: VertexOutput;
    // The draw's transform (identity for instanced solids) over the instance's own.
    let model_matrix = entity.model_matrix * instances[instance].model_matrix;

    // Bone skinning transform: the draw's run of `bones` (#455), blended per vertex.
    let joints = model.joint_indices + vec4<u32>(entity.bone_base);
    let bone_transform = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );

    let local_pos = bone_transform * vec4<f32>(model.position, 1.0);
    let world_pos = model_matrix * local_pos;
    
    // Normal transform
    let local_normal = bone_transform * vec4<f32>(model.normal, 0.0);
    let world_normal = normalize((model_matrix * local_normal).xyz);

    // Tangent rides the same skin + model transform as the normal; its `w`
    // (handedness) passes through untouched.
    let local_tangent = bone_transform * vec4<f32>(model.tangent.xyz, 0.0);
    let world_tangent = normalize((model_matrix * local_tangent).xyz);

    out.world_position = world_pos.xyz;
    out.world_normal = world_normal;
    out.tex_coords = model.tex_coords;
    out.world_tangent = vec4<f32>(world_tangent, model.tangent.w);
    out.clip_position = camera.view_proj * world_pos;
    out.instance = instance;
    let st = instances[instance].lightmap_st;
    out.lightmap_uv = model.lightmap_uv * st.xy + st.zw;
    return out;
}

const PI: f32 = 3.14159265359;
// RGBM's range (#438); mirrors `RGBM_RANGE` in src/scene/lighting/lightmap/encode.rs.
const LIGHTMAP_RGBM_RANGE: f32 = 8.0;

// Directional lightmaps (#810): reshape baked irradiance by how the shading normal `N`
// faces the dominant incoming direction, relative to the geometric normal `Ng` the
// bake saw (Unity's half-Lambert rebalance). `dir` is the decoded direction page texel:
// direction scaled by directionality, so a texel lit evenly from all round (0) or a
// surface whose normal map is flat (N == Ng) keeps its lightmap exactly.
fn lightmap_direction_rebalance(dir: vec3<f32>, N: vec3<f32>, Ng: vec3<f32>) -> f32 {
    let shaded = dot(N, dir) * 0.5 + 0.5;
    let baked = dot(Ng, dir) * 0.5 + 0.5;
    return shaded / max(baked, 1e-3);
}

// A direction page texel back to the direction scaled by its directionality; mirrors
// `decode_direction` in src/scene/lighting/lightmap/encode.rs.
fn decode_lightmap_direction(texel: vec4<f32>) -> vec3<f32> {
    let d = texel.xyz * 2.0 - 1.0;
    let len = length(d);
    if (len < 1e-4) {
        return vec3<f32>(0.0);
    }
    return d / len * texel.a;
}

fn DistributionGGX(N: vec3<f32>, H: vec3<f32>, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

    let nom   = a2;
    let denom = (NdotH2 * (a2 - 1.0) + 1.0);
    return nom / (PI * denom * denom);
}

fn GeometrySchlickGGX(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;

    let nom   = NdotV;
    let denom = NdotV * (1.0 - k) + k;

    return nom / denom;
}

fn GeometrySmith(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx2 = GeometrySchlickGGX(NdotV, roughness);
    let ggx1 = GeometrySchlickGGX(NdotL, roughness);

    return ggx2 * ggx1;
}

fn FresnelSchlick(cosTheta: f32, F0: vec3<f32>) -> vec3<f32> {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn calculate_pbr(
    N: vec3<f32>, 
    V: vec3<f32>, 
    L: vec3<f32>, 
    radiance: vec3<f32>, 
    F0: vec3<f32>, 
    metallic: f32, 
    roughness: f32, 
    albedo: vec3<f32>
) -> vec3<f32> {
    let H = normalize(L + V);
    let NdotL_raw = dot(N, L);
    
    // Half-Lambert diffuse wrapping (retro CS 1.6 / GoldSrc look)
    let half_lambert = pow(NdotL_raw * 0.5 + 0.5, 2.0);
    
    // Standard lighting factors for specular and math stability
    let NdotL = max(NdotL_raw, 0.0);
    let NdotV = max(dot(N, V), 0.0);

    let D = DistributionGGX(N, H, roughness);
    let G = GeometrySmith(N, V, L, roughness);
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0);

    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let numerator = D * G * F;
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    let specular = numerator / denominator;

    let diffuse = kD * albedo / PI;

    return diffuse * radiance * half_lambert + specular * radiance * NdotL;
}

// Percentage-closer filtering (PCF) kernel radius, in texels. The kernel is a
// square of side (2 * PCF_RADIUS + 1): radius 1 -> 3x3 (9 taps), radius 2 -> 5x5
// (25 taps). Raising it widens the penumbra and softens the shadow edge at a
// linear cost in samples. This is the tunable "sample count" knob for the soft
// shadow filter; bump it for softer edges, drop it for sharper/cheaper shadows.
const PCF_RADIUS: i32 = 2;

// Bias against self-shadowing, in texels of the cascade sampled: the surface is
// pushed off along its normal (more at grazing light) and its depth pulled toward
// the light. In texels, so every cascade gets the same relative bias however much
// ground its texels cover.
const NORMAL_OFFSET_TEXELS: f32 = 1.5;
const DEPTH_BIAS_TEXELS: f32 = 1.0;

// PCF over one cascade; 1.0 (lit) outside its volume.
fn sample_cascade(c: i32, world_pos: vec3<f32>, N: vec3<f32>, NdotL: f32) -> f32 {
    let texel = shadow.texels[c];
    let offset_pos = world_pos + N * texel * NORMAL_OFFSET_TEXELS * (1.0 - NdotL);
    let light_space_pos = shadow.light_space[c] * vec4<f32>(offset_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;
    let uv = vec2<f32>(proj_coords.x * 0.5 + 0.5, -proj_coords.y * 0.5 + 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || proj_coords.z > 1.0) {
        return 1.0;
    }
    let current_depth = proj_coords.z - texel * DEPTH_BIAS_TEXELS / shadow.depth_ranges[c];

    let size = textureDimensions(t_shadow);
    let texel_size = vec2<f32>(1.0 / f32(size.x), 1.0 / f32(size.y));

    // NxN PCF: average the hardware depth comparisons over a square texel
    // neighbourhood. Each tap is already bilinearly filtered by the comparison
    // sampler, so this stacks a wider blur on top of hardware PCF for a soft,
    // FEAR-era shadow edge instead of a single hard step. `Level` sampling: the
    // cascade is picked per fragment, so this runs in non-uniform control flow.
    var lit = 0.0;
    var taps = 0.0;
    for (var x = -PCF_RADIUS; x <= PCF_RADIUS; x = x + 1) {
        for (var y = -PCF_RADIUS; y <= PCF_RADIUS; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            lit += textureSampleCompareLevel(t_shadow, s_shadow, uv + offset, c, current_depth);
            taps += 1.0;
        }
    }
    return lit / taps;
}

// The sun's visibility at `world_pos`: the cascade whose slice holds the fragment's
// view depth, cross-faded into the next over the last `blend` of its range, and
// faded out over the last tenth of the shadow distance so the edge never pops.
fn calculate_shadow(world_pos: vec3<f32>, N: vec3<f32>) -> f32 {
    let count = i32(shadow.params.x);
    let distance = shadow.params.y;
    let depth = dot(world_pos - shadow.view_pos.xyz, shadow.view_dir.xyz);
    if (count == 0 || depth >= distance) {
        return 1.0;
    }
    var c = 0;
    while (c < count - 1 && depth > shadow.splits[c]) {
        c = c + 1;
    }
    let NdotL = clamp(dot(N, normalize(-lighting.dir_lights[0].direction)), 0.0, 1.0);
    var lit = sample_cascade(c, world_pos, N, NdotL);

    let blend_start = shadow.splits[c] * (1.0 - shadow.params.z);
    if (c < count - 1 && depth > blend_start) {
        let t = (depth - blend_start) / (shadow.splits[c] - blend_start);
        lit = mix(lit, sample_cascade(c + 1, world_pos, N, NdotL), t);
    }
    let fade = clamp((depth - distance * 0.9) / (distance * 0.1), 0.0, 1.0);
    return mix(lit, 1.0, fade);
}

// Reconstruct diffuse irradiance from the entity's L2 SH probe for a surface normal
// (#240). Mirrors `Sh9::eval` in src/scene/sh.rs byte-for-byte: the same 9 real-SH
// basis polynomials and the same cosine-lobe band factors (pi, 2pi/3, pi/4), so the
// headless analytic path and the GPU agree. Result clamped to non-negative.
fn eval_sh(N: vec3<f32>, instance: u32) -> vec3<f32> {
    let x = N.x;
    let y = N.y;
    let z = N.z;
    var b: array<f32, 9>;
    b[0] = 0.2820948;
    b[1] = -0.4886025 * y;
    b[2] = 0.4886025 * z;
    b[3] = -0.4886025 * x;
    b[4] = 1.0925484 * x * y;
    b[5] = -1.0925484 * y * z;
    b[6] = 0.31539157 * (3.0 * z * z - 1.0);
    b[7] = -1.0925484 * x * z;
    b[8] = 0.5462742 * (x * x - y * y);
    let a0 = 3.1415927;
    let a1 = 2.0943952; // 2*pi/3
    let a2 = 0.7853982; // pi/4
    var lobe: array<f32, 9>;
    lobe[0] = a0;
    lobe[1] = a1; lobe[2] = a1; lobe[3] = a1;
    lobe[4] = a2; lobe[5] = a2; lobe[6] = a2; lobe[7] = a2; lobe[8] = a2;
    var out = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        out += instances[instance].sh[i].xyz * b[i] * lobe[i];
    }
    return max(out, vec3<f32>(0.0));
}

// Box-projected parallax correction (#244), the standard Lagarde technique. Given a
// fragment world position and a raw reflection direction, intersect the ray against the
// active probe's box and return the direction from the probe centre to that hit point —
// so the reflection tracks the room as the camera moves. Mirrors the Rust
// `ReflectionProbe::parallax_correct` byte-for-byte (unit-tested there without a GPU).
fn parallax_correct(world_pos: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    let inv = vec3<f32>(1.0) / dir;
    let first = (lighting.refl_box_max.xyz - world_pos) * inv;
    let second = (lighting.refl_box_min.xyz - world_pos) * inv;
    let furthest = max(first, second);
    let t = min(min(furthest.x, furthest.y), furthest.z);
    if (t <= 0.0) {
        return dir;
    }
    let hit = world_pos + dir * t;
    return normalize(hit - lighting.refl_center.xyz);
}

// The environment a surface reflects along the mirror direction `R` (#244, #718). The
// raw direction is parallax-corrected against the active reflection probe's box when one
// covers the fragment, so the reflection tracks the room as the camera moves rather than
// behaving like an infinitely-distant sky. Then the source, in order: the probe's baked,
// prefiltered cube (roughness selects the mip, #245); the skybox panorama; and with
// neither, the same procedural sky the sky pass draws (#256), so the reflection matches
// what is on screen. (Roughness blurs only the baked cube; the 2D skies stay sharp until
// #245 prefilters them.)
fn environment_reflection(world_pos: vec3<f32>, raw_r: vec3<f32>, roughness: f32) -> vec3<f32> {
    var R = normalize(raw_r);
    if (lighting.refl_active > 0.5) {
        R = parallax_correct(world_pos, R);
    }
    if (lighting.refl_has_cubemap > 0.5) {
        let max_mip = f32(textureNumLevels(t_refl_cube) - 1u);
        return textureSampleLevel(t_refl_cube, s_refl_cube, R, roughness * max_mip).rgb;
    }
    if (lighting.sky_textured > 0.5) {
        // The equirectangular panorama, mapped as the skybox pass maps it.
        let phi = atan2(R.z, R.x);
        let theta = acos(clamp(R.y, -1.0, 1.0));
        return textureSample(t_skybox, s_skybox, vec2<f32>((phi + PI) / (2.0 * PI), theta / PI)).rgb;
    }
    return procedural_sky(lighting.ambient.color, R);
}

// The SSAO depth prepass (#436): depth only, so it writes no colour. Cutout texels
// are clipped as `fs_main` clips them, and unlit draws (gizmos, grids) are skipped:
// neither should cast occlusion. A surface variant whose blocks cut fragments
// (`dissolve`) adds `fs_prepass_cut`, which runs its cuts and then this (#648).
fn prepass_clip(in: VertexOutput) {
    var alpha = entity.color_tint.a;
    if (entity.use_texture == 1u) {
        alpha *= textureSample(t_diffuse, s_diffuse, in.tex_coords).a;
    }
    if (entity.is_lit == 0u || (entity.use_cutout == 1u && alpha < entity.alpha_cutoff)) {
        discard;
    }
}

@fragment
fn fs_prepass(in: VertexOutput) {
    prepass_clip(in);
}

// ---- The shadow pass (#435, #648) ----
// The cascades' depth is drawn from this module too, so a surface variant carries
// its own shadow entry points. Its resources sit at bindings the forward pass leaves
// free — group 0 binding 6, group 1 binding 3 — and share `bones` (group 1 binding 1)
// with it, so one module holds both without a clash. Group 2 is the material's, as
// in the forward pass: only the clipping pipelines bind it.

struct ShadowLight {
    light_space: mat4x4<f32>,
};

// The cascade being drawn: one matrix per cascade, picked by dynamic offset.
@group(0) @binding(6)
var<uniform> shadow_light: ShadowLight;

// One caster (#470): its world matrix, where its joint palette starts in `bones`
// (#599; 0, the shared identity, for a mesh with no skin), and its cutout alpha test
// (#648): the albedo map's alpha when `use_texture == 1` (a cutout's tint alpha is
// always 1), clipped below `alpha_cutoff` (0 for a material that does not clip).
// Mirrors `CasterData` (src/render/passes/shadows/casters/buffer.rs) byte-for-byte.
struct Caster {
    model: mat4x4<f32>,
    bone_base: u32,
    alpha_cutoff: f32,
    use_texture: u32,
    _pad0: u32,
};

// Every caster, one per instance: a draw covers a run of casters sharing one mesh
// (and material, when it clips), indexed by `instance_index`.
@group(1) @binding(3)
var<storage, read> casters: array<Caster>;

// A caster's vertex in the cascade's light space. Only what a clip reads is filled:
// the UVs and world position; the normal and tangent stay zero.
@vertex
fn vs_shadow(model: VertexInput, @builtin(instance_index) instance: u32) -> VertexOutput {
    let caster = casters[instance];
    let joints = model.joint_indices + vec4<u32>(caster.bone_base);
    let skin = blend_joints(
        bones[joints.x], bones[joints.y], bones[joints.z], bones[joints.w],
        model.joint_weights,
    );
    let world = caster.model * skin * vec4<f32>(model.position, 1.0);
    var out: VertexOutput;
    out.clip_position = shadow_light.light_space * world;
    out.world_position = world.xyz;
    out.tex_coords = model.tex_coords;
    out.instance = instance;
    return out;
}

// A cutout caster's shadow is clipped as its surface is (#648). The albedo is sampled
// unconditionally: `use_texture` is per instance, so a branch on it is not uniform.
fn shadow_clip(in: VertexOutput) {
    let caster = casters[in.instance];
    let texel = textureSample(t_diffuse, s_diffuse, in.tex_coords).a;
    let alpha = select(1.0, texel, caster.use_texture == 1u);
    if (alpha < caster.alpha_cutoff) {
        discard;
    }
}

@fragment
fn fs_shadow(in: VertexOutput) {
    shadow_clip(in);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_color: vec4<f32>;
    if (entity.use_texture == 1u) {
        base_color = textureSample(t_diffuse, s_diffuse, in.tex_coords) * entity.color_tint;
    } else {
        base_color = entity.color_tint;
    }

    // Cutout alpha-test (#242): for a Cutout material, drop sub-threshold fragments
    // before any shading so the surface reads as hard-edged (foliage, grates). Opaque
    // and Transparent materials leave `use_cutout == 0` and skip this. Done before the
    // unlit early-out so an unlit cutout (a flat alpha-tested billboard) also clips.
    if (entity.use_cutout == 1u && base_color.a < entity.alpha_cutoff) {
        discard;
    }

    // Unlit rendering (e.g. grids, path highlights, light gizmos). Still fogged: an
    // unlit sign down a smoky corridor fades like the wall it hangs on (#437).
    if (entity.is_lit == 0u) {
        return vec4<f32>(apply_fog(camera.fog, base_color.rgb, in.world_position, camera.camera_pos), base_color.a);
    }

    // Geometric normal, optionally perturbed by a tangent-space normal map (#207).
    // The map's RGB in [0,1] decodes to a [-1,1] vector; the TBN basis rotates it
    // into world space. `w` (handedness) flips the bitangent for mirrored UVs.
    var N = normalize(in.world_normal);
    if (entity.use_normal_map == 1u) {
        let T = normalize(in.world_tangent.xyz);
        let B = cross(N, T) * in.world_tangent.w;
        let sampled = textureSample(t_normal, s_diffuse, in.tex_coords).xyz * 2.0 - 1.0;
        N = normalize(mat3x3<f32>(T, B, N) * sampled);
    }
    let V = normalize(camera.camera_pos - in.world_position);

    // glTF metallic-roughness packs metallic in BLUE, roughness in GREEN. Each scalar
    // multiplies its map channel when the flag is set, else acts alone (#202).
    let surface_metallic = clamp(entity.metallic * select(1.0, textureSample(t_metallic, s_diffuse, in.tex_coords).b, entity.use_metallic_map == 1u), 0.0, 1.0);
    let surface_roughness = clamp(entity.roughness * select(1.0, textureSample(t_roughness, s_diffuse, in.tex_coords).g, entity.use_roughness_map == 1u), 0.04, 1.0);

    // 0. Surface decals (#638): this cluster's decals change the material inputs
    // before any light is summed, so a decal is lit, shadowed and occluded like the
    // surface it lands on. The angle fade reads the geometric normal.
    let cluster = cluster_index(camera.clusters, camera.view_proj, in.world_position);
    var surface = DecalSurface(base_color.rgb, N, surface_metallic, surface_roughness, 1.0);
    if (entity.receive_decals == 1u) {
        surface = fold_decals(surface, cluster, in.world_position, normalize(in.world_normal));
    }
    N = surface.normal;
    let albedo = surface.albedo;
    let metallic = surface.metallic;
    let roughness = surface.roughness;

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    // 1. Ambient lighting term. A lightmapped static mesh reads its lightmap (#438).
    // Non-static objects that a light probe covers
    // (`use_sh == 1`, #240) reconstruct DIRECTIONAL irradiance from their interpolated
    // SH probe; everything else falls back to the flat Hemispherical Sky-Ground
    // gradient. All three are E / pi, so they feed the same albedo * (1 - metallic)
    // Lambert response.
    var ambient_irradiance: vec3<f32>;
    let lightmap_page = instances[in.instance].lightmap_page;
    if (lightmap_page > 0u) {
        // Baked lightmap (#438): bounce, sky, emission and `Baked` lights' direct light,
        // stored RGBM as E / pi, so `* albedo` is the Lambert response a realtime light
        // gives. `Level` sampling: the page is per instance, so this branch may diverge.
        let lm = textureSampleLevel(t_lightmaps, s_lightmaps, in.lightmap_uv, lightmap_page - 1u, 0.0);
        ambient_irradiance = lm.rgb * lm.a * LIGHTMAP_RGBM_RANGE;
        if (instances[in.instance].lightmap_directional == 1u) {
            // The normal-mapped (and decal-perturbed) N reshapes the baked light.
            let texel = textureSampleLevel(t_lightmap_dirs, s_lightmaps, in.lightmap_uv, lightmap_page - 1u, 0.0);
            let dir = decode_lightmap_direction(texel);
            ambient_irradiance *= lightmap_direction_rebalance(dir, N, normalize(in.world_normal));
        }
    } else if (instances[in.instance].use_sh == 1u) {
        // `eval_sh` returns irradiance E; a Lambert surface reflects albedo / pi * E,
        // the same response `calculate_pbr` gives a direct light and the E / pi the
        // lightmap and flat ambient already hold (#807).
        ambient_irradiance = eval_sh(N, in.instance) / PI;
    } else {
        let sky_color = lighting.ambient.color;
        let ground_color = sky_color * 0.25; // ground is darker and cooler/desaturated
        let ambient_grad = mix(ground_color, sky_color, N.y * 0.5 + 0.5);
        ambient_irradiance = ambient_grad * lighting.ambient.intensity;
    }
    // SSAO (#436) darkens only the indirect light: ambient here, env reflection below.
    let ao = ambient_occlusion(in.clip_position.xy) * surface.occlusion;
    var lighting_color = ambient_irradiance * albedo * (1.0 - metallic) * ao;

    // A lightmapped surface skips `Baked` lights below: their direct light is already
    // in its lightmap (#438). Only a probe/reflection bake capture uploads them at all
    // (#809); a frame draws `Mixed` and `Realtime` lights only.
    let skip_baked = lightmap_page > 0u;

    // 2. Directional lights (#434). Slot 0 is the sun, the one the cascades shadow.
    let shadow = calculate_shadow(in.world_position, N);
    for (var i = 0u; i < lighting.num_dir_lights; i = i + 1u) {
        let sun = lighting.dir_lights[i];
        if (skip_baked && sun.baked > 0.5) {
            continue;
        }
        let radiance = sun.color * sun.intensity * select(1.0, shadow, i == 0u);
        lighting_color += calculate_pbr(N, V, normalize(-sun.direction), radiance, F0, metallic, roughness, albedo);
    }

    // 3. Point and spot lights, only those binned into this fragment's cluster (#434),
    // each shadowed through the atlas when it casts (#468).
    let range = cluster_ranges[cluster];
    for (var i = 0u; i < range.y; i = i + 1u) {
        let light = local_lights[cluster_lights[range.x + i]];
        if (skip_baked && light.baked > 0.5) {
            continue;
        }
        var radiance = local_light_radiance(light, in.world_position);
        if (all(radiance == vec3<f32>(0.0))) {
            continue;
        }
        // Its shadow, when the atlas holds one (#468).
        let tile = local_shadow_tile(light, in.world_position);
        if (tile != NO_SHADOW) {
            radiance *= sample_local_shadow(shadow_tiles[tile], t_shadow_atlas, s_shadow, light.position, in.world_position, N);
        }
        let L = normalize(light.position - in.world_position);
        lighting_color += calculate_pbr(N, V, L, radiance, F0, metallic, roughness, albedo);
    }

    // 5. Environment reflections (#244), always on (#718): like Unity's Lighting ->
    // Environment Reflections, the sky / reflection-probe term applies whether or not
    // the camera runs SSR, which only adds screen-space detail on top (postfx). A metal
    // has no diffuse term, so without this it renders black. See `environment_reflection`.
    let R = reflect(-V, N);
    let F_refl = FresnelSchlick(max(dot(N, V), 0.0), F0);
    let reflection_scale = (1.0 - roughness) * (metallic + (1.0 - metallic) * 0.2);
    lighting_color += environment_reflection(in.world_position, R, roughness) * F_refl * reflection_scale * ao;

    // 6. Emissive (#222 factor, #207 map): self-illumination added on top of the lit
    // colour, independent of any light. The factor is modulated by the emissive map's
    // rgb when one is bound (`factor * map.rgb`, the glTF convention). The HDR target +
    // bloom bright-pass turn values >1.0 into a glow with no post-FX work here.
    var emissive = entity.emissive.rgb;
    if (entity.use_emissive_map == 1u) {
        emissive *= textureSample(t_emissive, s_diffuse, in.tex_coords).rgb;
    }
    lighting_color += emissive;

    // 7. Scene fog (#437), last, in linear HDR before post-FX. Authored surface
    // blocks fold into `lighting_color` here, so they are fogged too.
    return vec4<f32>(apply_fog(camera.fog, lighting_color, in.world_position, camera.camera_pos), base_color.a);
}
