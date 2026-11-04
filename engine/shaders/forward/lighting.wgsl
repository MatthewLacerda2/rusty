// engine/shaders/forward/lighting.wgsl — the forward pass's lighting helpers (#1015):
// decals, ambient occlusion, lightmaps, the PBR response, the sun's cascaded shadow,
// probe SH and the flat ambient. `fs_main` in `shader.wgsl` sums them, with the
// environment reflection of `forward::reflection`.

#define_import_path forward::lighting

#import common::{DecalSurface, PI, apply_decal}
#import lighting::ambient::ambient_radiance
#import lighting::sh::sh_basis
#import forward::proxy_volume::proxy_irradiance
#import forward::bindings::{camera, cluster_lights, cluster_ranges, decals, instances, lighting, s_decal, s_shadow, shadow, t_ao, t_decal_color, t_decal_data, t_lightmaps, t_shadow}

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

// The mip level of the lightmap pages that `uv` reads (#1694): the larger screen
// footprint of one pixel, in page texels, as a GPU's own implicit LOD picks it. The
// derivatives are taken here, so call it before any per-pixel branch
// (src/render/CLAUDE.md); the sample itself then reads with `textureSampleLevel`,
// which is safe in the per-instance branch that follows.
fn lightmap_level(uv: vec2<f32>) -> f32 {
    let edge = f32(textureDimensions(t_lightmaps).x);
    let levels = f32(textureNumLevels(t_lightmaps));
    let dx = dpdx(uv) * edge;
    let dy = dpdy(uv) * edge;
    let footprint = max(dot(dx, dx), dot(dy, dy));
    return clamp(0.5 * log2(max(footprint, 1e-8)), 0.0, levels - 1.0);
}

// Directional lightmaps (#810): reshape baked irradiance by how the shading normal `N`
// faces the dominant incoming direction, relative to the geometric normal `Ng` the
// bake saw (Unity's half-Lambert rebalance). `dir` is the decoded direction page
// texel's vector: direction scaled by directionality, so a surface whose normal map
// is flat (N == Ng) keeps its lightmap exactly at any direction, and a texel lit with
// no dominant direction (0, which an open surface never quite reaches: an even sky
// gives 2/3) keeps it too. Where `Baked` lamps light the texel, the direction is
// theirs (`bake::direction`), so a normal map reshapes the bounce there by the lamp.
fn lightmap_direction_rebalance(dir: vec3<f32>, N: vec3<f32>, Ng: vec3<f32>) -> f32 {
    let shaded = dot(N, dir) * 0.5 + 0.5;
    let baked = dot(Ng, dir) * 0.5 + 0.5;
    return shaded / max(baked, 1e-3);
}

// A direction page texel back to its vector (`xyz`, direction scaled by
// directionality) and its lamp share (`w`, #1695); mirrors `decode_direction` in
// src/scene/lighting/lightmap/encode.rs, which stores each component as 128 + 127x.
fn decode_lightmap_direction(texel: vec4<f32>) -> vec4<f32> {
    return vec4<f32>((texel.xyz * 255.0 - 128.0) / 127.0, texel.a);
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

// The GGX specular BRDF (Cook-Torrance: D * G * F / (4 N·V N·L)) for light from `L`
// seen from `V`, given its Fresnel `F` at the half vector. Shared by every live light
// (`calculate_pbr`) and the directional lightmap's one baked lobe
// (`forward::lightmap::baked_specular`, #1695), so the two highlights agree.
fn ggx_specular(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, H: vec3<f32>, F: vec3<f32>, roughness: f32) -> vec3<f32> {
    let NdotL = max(dot(N, L), 0.0);
    let NdotV = max(dot(N, V), 0.0);
    let D = DistributionGGX(N, H, roughness);
    let G = GeometrySmith(N, V, L, roughness);
    let numerator = D * G * F;
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    return numerator / denominator;
}

// What a light of `radiance` arriving from `L` reflects specularly toward `V`: the
// GGX lobe times N·L, with no diffuse term.
fn specular_response(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, radiance: vec3<f32>, F0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let H = normalize(L + V);
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0);
    return ggx_specular(N, V, L, H, F, roughness) * radiance * max(dot(N, L), 0.0);
}

// The share of a lamp's radiance a surface diffuses at `NdotL_raw` = N·L under the
// scene's diffuse model (#1698), the one the lightmap and probe bakes use too
// (`DiffuseModel::response`): Lambert, or the retro squared wrap.
fn diffuse_response(NdotL_raw: f32) -> f32 {
    let wrap = clamp(NdotL_raw * 0.5 + 0.5, 0.0, 1.0);
    return select(max(NdotL_raw, 0.0), wrap * wrap, lighting.diffuse_wrap > 0.5);
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
    let NdotL = max(dot(N, L), 0.0);
    // Diffuse follows the scene's model (`diffuse_response`); specular is always the
    // physical N·L.
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0);

    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let specular = ggx_specular(N, V, L, H, F, roughness);
    let diffuse = kD * albedo / PI;

    return diffuse * radiance * diffuse_response(dot(N, L)) + specular * radiance * NdotL;
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
// (#240). Mirrors `Sh9::eval` in src/scene/lighting/sh.rs: the same 9 real-SH
// basis polynomials (`lighting::sh::sh_basis`) and the same cosine-lobe band factors
// (pi, 2pi/3, pi/4), so the headless analytic path and the GPU agree. Result clamped
// to non-negative.
fn eval_sh(N: vec3<f32>, instance: u32) -> vec3<f32> {
    let b = sh_basis(N);
    let a0 = PI;
    let a1 = 2.0 * PI / 3.0;
    let a2 = PI / 4.0;
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

// The probe irradiance E at `world_pos` for normal `N`: the instance's one SH sample
// (`eval_sh`), or, with a Light Probe Proxy Volume, the volume's grid (#1672).
fn probe_irradiance(world_pos: vec3<f32>, N: vec3<f32>, instance: u32) -> vec3<f32> {
    let proxy = proxy_irradiance(world_pos, N, instance);
    return select(eval_sh(N, instance), proxy.rgb, proxy.w > 0.5);
}

// The scene's ambient at normal `N`, as irradiance E / pi (`lighting::ambient::ambient_radiance`:
// the `Color` hemisphere or the `Gradient` trilight, #1647). What a surface no
// lightmap or probe covers takes (`surface_color`'s step 1).
fn flat_ambient(N: vec3<f32>) -> vec3<f32> {
    return ambient_radiance(lighting.ambient, N);
}
