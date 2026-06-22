#import common::{CameraUniforms, VertexInput}

struct AmbientLight {
    color: vec3<f32>,
    intensity: f32,
};

struct DirectionalLight {
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    _pad: f32,
};

struct PointLight {
    position: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
};

struct Spotlight {
    position: vec3<f32>,
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    inner_cone: f32, // Cosine of inner angle
    outer_cone: f32, // Cosine of outer angle
};

struct LightingUniforms {
    ambient: AmbientLight,
    dir_light: DirectionalLight,
    point_lights: array<PointLight, 4>,
    spot_light: Spotlight,
    num_point_lights: u32,
    ssr_active: f32,
    ssr_quality: f32,
    ssr_temporal_upsampling: f32,
    // Active reflection probe (#244): when `refl_active > 0.5` the env reflection is
    // box-projected (Lagarde parallax) against this probe's box around `refl_center`
    // instead of sampled as an infinitely-distant skybox. Mirrors the Rust
    // `LightingUniform` byte-for-byte; the `.w` lanes are padding.
    refl_active: f32,
    _refl_pad0: f32,
    _refl_pad1: f32,
    _refl_pad2: f32,
    refl_center: vec4<f32>,
    refl_box_min: vec4<f32>,
    refl_box_max: vec4<f32>,
};

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
    // Light-probe SH (#240). When `use_sh == 1` the ambient term is reconstructed
    // from the 9 L2 SH coefficients below (xyz = RGB radiance, w unused) instead of
    // the flat hemispherical gradient. The three scalar pads mirror the Rust
    // struct's `[u32; 3]` byte-for-byte — a `vec3<u32>` here would be 16-aligned and
    // open a 12-byte hole, desyncing the layouts (288 vs 304 bytes).
    use_sh: u32,
    _sh_pad0: u32,
    _sh_pad1: u32,
    _sh_pad2: u32,
    sh: array<vec4<f32>, 9>,
};

struct BoneUniforms {
    bones: array<mat4x4<f32>, 64>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

@group(0) @binding(1)
var<uniform> lighting: LightingUniforms;

@group(0) @binding(2)
var t_skybox: texture_2d<f32>;
@group(0) @binding(3)
var s_skybox: sampler;

@group(1) @binding(0)
var<uniform> entity: EntityUniforms;

@group(1) @binding(1)
var<uniform> bones: BoneUniforms;

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

@group(3) @binding(0)
var<uniform> light_space: mat4x4<f32>;
@group(3) @binding(1)
var t_shadow: texture_depth_2d;
@group(3) @binding(2)
var s_shadow: sampler_comparison;


struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    // World-space tangent for normal mapping; `w` carries the handedness sign so the
    // fragment shader can reconstruct the bitangent as `w * cross(N, T)`.
    @location(3) world_tangent: vec4<f32>,
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Bone skinning transform
    var bone_transform = bones.bones[model.joint_indices.x] * model.joint_weights.x
                       + bones.bones[model.joint_indices.y] * model.joint_weights.y
                       + bones.bones[model.joint_indices.z] * model.joint_weights.z
                       + bones.bones[model.joint_indices.w] * model.joint_weights.w;

    // If bone weights are zero/uninitialized, default to identity matrix
    let total_weight = model.joint_weights.x + model.joint_weights.y + model.joint_weights.z + model.joint_weights.w;
    if (total_weight < 0.01) {
        bone_transform = mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
    }

    let local_pos = bone_transform * vec4<f32>(model.position, 1.0);
    let world_pos = entity.model_matrix * local_pos;
    
    // Normal transform
    let local_normal = bone_transform * vec4<f32>(model.normal, 0.0);
    let world_normal = normalize((entity.model_matrix * local_normal).xyz);

    // Tangent rides the same skin + model transform as the normal; its `w`
    // (handedness) passes through untouched.
    let local_tangent = bone_transform * vec4<f32>(model.tangent.xyz, 0.0);
    let world_tangent = normalize((entity.model_matrix * local_tangent).xyz);

    out.world_position = world_pos.xyz;
    out.world_normal = world_normal;
    out.tex_coords = model.tex_coords;
    out.world_tangent = vec4<f32>(world_tangent, model.tangent.w);
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

const PI: f32 = 3.14159265359;

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

fn calculate_shadow(world_pos: vec3<f32>, N: vec3<f32>) -> f32 {
    let light_space_pos = light_space * vec4<f32>(world_pos, 1.0);
    let proj_coords = light_space_pos.xyz / light_space_pos.w;

    let flip_y = vec3<f32>(
        proj_coords.x * 0.5 + 0.5,
        -proj_coords.y * 0.5 + 0.5,
        proj_coords.z
    );

    if (flip_y.x < 0.0 || flip_y.x > 1.0 || flip_y.y < 0.0 || flip_y.y > 1.0 || flip_y.z > 1.0) {
        return 1.0;
    }

    let bias = max(0.005 * (1.0 - dot(N, normalize(-lighting.dir_light.direction))), 0.0005);
    let current_depth = flip_y.z - bias;

    let size = textureDimensions(t_shadow);
    let texel_size = vec2<f32>(1.0 / f32(size.x), 1.0 / f32(size.y));

    // NxN PCF: average the hardware depth comparisons over a square texel
    // neighbourhood. Each tap is already bilinearly filtered by the comparison
    // sampler, so this stacks a wider blur on top of hardware PCF for a soft,
    // FEAR-era shadow edge instead of a single hard step.
    var shadow = 0.0;
    var taps = 0.0;
    for (var x = -PCF_RADIUS; x <= PCF_RADIUS; x = x + 1) {
        for (var y = -PCF_RADIUS; y <= PCF_RADIUS; y = y + 1) {
            let offset = vec2<f32>(f32(x), f32(y)) * texel_size;
            shadow += textureSampleCompare(t_shadow, s_shadow, flip_y.xy + offset, current_depth);
            taps += 1.0;
        }
    }
    shadow /= taps;

    return shadow;
}

// Reconstruct diffuse irradiance from the entity's L2 SH probe for a surface normal
// (#240). Mirrors `Sh9::eval` in src/scene/sh.rs byte-for-byte: the same 9 real-SH
// basis polynomials and the same cosine-lobe band factors (pi, 2pi/3, pi/4), so the
// headless analytic path and the GPU agree. Result clamped to non-negative.
fn eval_sh(N: vec3<f32>) -> vec3<f32> {
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
        out += entity.sh[i].xyz * b[i] * lobe[i];
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var base_color: vec4<f32>;
    if (entity.use_texture == 1u) {
        base_color = textureSample(t_diffuse, s_diffuse, in.tex_coords) * entity.color_tint;
    } else {
        base_color = entity.color_tint;
    }

    // Unlit rendering (e.g. grids, path highlights, light gizmos)
    if (entity.is_lit == 0u) {
        return base_color;
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

    let albedo = base_color.rgb;
    // glTF metallic-roughness packs metallic in BLUE, roughness in GREEN. Each scalar
    // multiplies its map channel when the flag is set, else acts alone (#202).
    let metallic = clamp(entity.metallic * select(1.0, textureSample(t_metallic, s_diffuse, in.tex_coords).b, entity.use_metallic_map == 1u), 0.0, 1.0);
    let roughness = clamp(entity.roughness * select(1.0, textureSample(t_roughness, s_diffuse, in.tex_coords).g, entity.use_roughness_map == 1u), 0.04, 1.0);

    let F0 = mix(vec3<f32>(0.04), albedo, metallic);

    // 1. Ambient lighting term. Non-static objects that a light probe covers
    // (`use_sh == 1`, #240) reconstruct DIRECTIONAL irradiance from their interpolated
    // SH probe; everything else falls back to the flat Hemispherical Sky-Ground
    // gradient. Both feed the same albedo * (1 - metallic) diffuse response.
    var ambient_irradiance: vec3<f32>;
    if (entity.use_sh == 1u) {
        ambient_irradiance = eval_sh(N);
    } else {
        let sky_color = lighting.ambient.color;
        let ground_color = sky_color * 0.25; // ground is darker and cooler/desaturated
        let ambient_grad = mix(ground_color, sky_color, N.y * 0.5 + 0.5);
        ambient_irradiance = ambient_grad * lighting.ambient.intensity;
    }
    var lighting_color = ambient_irradiance * albedo * (1.0 - metallic);

    // 2. Directional Light
    let L_dir = normalize(-lighting.dir_light.direction);
    let radiance_dir = lighting.dir_light.color * lighting.dir_light.intensity;
    let shadow = calculate_shadow(in.world_position, N);
    lighting_color += calculate_pbr(N, V, L_dir, radiance_dir, F0, metallic, roughness, albedo) * shadow;


    // 3. Point Lights
    for (var i = 0u; i < lighting.num_point_lights; i = i + 1u) {
        let light = lighting.point_lights[i];
        let light_dir = light.position - in.world_position;
        let d = length(light_dir);
        if (d > light.range) {
            continue;
        }
        let L = normalize(light_dir);
        let atten = 1.0 / (d * d + 1.0);
        let radiance = light.color * light.intensity * atten;
        lighting_color += calculate_pbr(N, V, L, radiance, F0, metallic, roughness, albedo);
    }

    // 4. Spotlight
    let spot = lighting.spot_light;
    let spot_dir = spot.position - in.world_position;
    let spot_dist = length(spot_dir);
    if (spot_dist <= spot.range) {
        let L_spot = normalize(spot_dir);
        let theta = dot(L_spot, normalize(-spot.direction));
        
        if (theta > spot.outer_cone) {
            let intensity = clamp((theta - spot.outer_cone) / (spot.inner_cone - spot.outer_cone), 0.0, 1.0);
            let atten = 1.0 / (spot_dist * spot_dist + 1.0);
            let radiance = spot.color * spot.intensity * atten * intensity;
            lighting_color += calculate_pbr(N, V, L_spot, radiance, F0, metallic, roughness, albedo);
        }
    }

    // 5. Environment reflections (#244). The raw mirror direction is parallax-corrected
    // against the active reflection probe's box when one covers the fragment, so the
    // reflected environment tracks the room as the camera moves rather than behaving
    // like an infinitely-distant skybox. With no probe (or no SSR) it samples the global
    // skybox directly, the prior behaviour. (Roughness will select a prefiltered cubemap
    // mip once #245 bakes the chain; today the LDR skybox is the env source.)
    if (lighting.ssr_active > 0.5) {
        var R = reflect(-V, N);
        if (lighting.refl_active > 0.5) {
            R = parallax_correct(in.world_position, normalize(R));
        }
        let phi = atan2(R.z, R.x);
        let theta = acos(clamp(R.y, -1.0, 1.0));
        let u = (phi + PI) / (2.0 * PI);
        let v = theta / PI;
        let env_reflection = textureSample(t_skybox, s_skybox, vec2<f32>(u, v)).rgb;

        let F_refl = FresnelSchlick(max(dot(N, V), 0.0), F0);
        let reflection_scale = (1.0 - roughness) * (metallic + (1.0 - metallic) * 0.2);
        lighting_color += env_reflection * F_refl * reflection_scale;
    }

    // 6. Emissive (#222 factor, #207 map): self-illumination added on top of the lit
    // colour, independent of any light. The factor is modulated by the emissive map's
    // rgb when one is bound (`factor * map.rgb`, the glTF convention). The HDR target +
    // bloom bright-pass turn values >1.0 into a glow with no post-FX work here.
    var emissive = entity.emissive.rgb;
    if (entity.use_emissive_map == 1u) {
        emissive *= textureSample(t_emissive, s_diffuse, in.tex_coords).rgb;
    }
    lighting_color += emissive;

    return vec4<f32>(lighting_color, base_color.a);
}
