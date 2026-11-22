
#define_import_path forward::lighting

#import common::{DecalSurface, PI, apply_decal}
#import lighting::ambient::ambient_radiance
#import lighting::sh::sh_basis
#import forward::proxy_volume::proxy_irradiance
#import forward::transparent
#import forward::bindings::{camera, cluster_lights, cluster_ranges, decals, lighting, objects, s_decal, s_shadow, shadow, t_ao, t_decal_color, t_decal_data, t_lightmaps, t_opaque, t_shadow}

// Derivatives are taken here, before the cluster's own loop, so the walk may diverge
// per pixel.
fn fold_decals(s: DecalSurface, cluster: u32, world: vec3<f32>, facing: vec3<f32>) -> DecalSurface {
    let footprint = exp2(camera.lod.x);
    let dx = dpdx(world) * footprint;
    let dy = dpdy(world) * footprint;
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

// E / π. Only the opaque pass of a base camera running SSIL reads it (`camera.ssil.x`);
// other passes get none, so the branch is uniform.
fn indirect_bounce(frag: vec2<f32>) -> vec3<f32> {
    if (camera.ssil.x <= 0.5) {
        return vec3<f32>(0.0);
    }
    let last = textureDimensions(t_opaque) - vec2<u32>(1u);
    return textureLoad(t_opaque, min(vec2<u32>(frag), last), 0).rgb;
}

var<private> ssil_realtime: vec3<f32>;

fn lamp(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, front: vec3<f32>, back: vec3<f32>, F0: vec3<f32>, F90: f32, metallic: f32, roughness: f32, albedo: vec3<f32>, transmission: vec4<f32>, bounce_baked: f32) -> vec3<f32> {
    var c = calculate_pbr(N, V, L, front, F0, F90, metallic, roughness, albedo);
    if (transmission.w > 0.0) {
        c += transparent::transmitted(N, L, back, albedo, transmission.rgb);
    }
    ssil_realtime += c * (1.0 - bounce_baked);
    return c;
}

// By the brightest channel, so a coloured light keeps its weight; 0 for a black pixel.
fn realtime_share(lit: vec3<f32>, realtime: vec3<f32>) -> f32 {
    let whole = max(lit.r, max(lit.g, lit.b));
    let part = max(realtime.r, max(realtime.g, realtime.b));
    return select(0.0, saturate(part / whole), whole > 0.0);
}

// `camera.ssil.x` is set for that opaque pass alone, so translucents after it keep their
// blend alpha.
fn with_ssil_share(c: vec4<f32>) -> vec4<f32> {
    if (camera.ssil.x <= 0.5) {
        return c;
    }
    return vec4<f32>(c.rgb, realtime_share(c.rgb, ssil_realtime));
}

// Call before any per-pixel branch (derivatives); the sample then reads with
// `textureSampleLevel`.
fn lightmap_level(uv: vec2<f32>) -> f32 {
    let edge = f32(textureDimensions(t_lightmaps).x);
    let levels = f32(textureNumLevels(t_lightmaps));
    let dx = dpdx(uv) * edge;
    let dy = dpdy(uv) * edge;
    let footprint = max(dot(dx, dx), dot(dy, dy));
    return clamp(0.5 * log2(max(footprint, 1e-8)), 0.0, levels - 1.0);
}

// Unity's half-Lambert rebalance. A flat normal map (N == Ng) or a texel with no
// dominant direction keeps its lightmap exactly.
fn lightmap_direction_rebalance(dir: vec3<f32>, N: vec3<f32>, Ng: vec3<f32>) -> f32 {
    let shaded = dot(N, dir) * 0.5 + 0.5;
    let baked = dot(Ng, dir) * 0.5 + 0.5;
    // Bytes round, so the decoded vector can be a hair over 1 and `shaded` dip under 0
    // behind a lamp.
    return max(shaded, 0.0) / max(baked, 1e-3);
}

// Mirrors `scene::lighting::page::encode::decode_direction`: 128 + 127x per component.
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

// `F90` is `KHR_materials_specular`'s `specular` for a dielectric, so one at `specular`
// 0 reflects nothing at any angle.
fn FresnelSchlick(cosTheta: f32, F0: vec3<f32>, F90: f32) -> vec3<f32> {
    return F0 + (F90 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

fn ggx_specular(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, H: vec3<f32>, F: vec3<f32>, roughness: f32) -> vec3<f32> {
    let NdotL = max(dot(N, L), 0.0);
    let NdotV = max(dot(N, V), 0.0);
    let D = DistributionGGX(N, H, roughness);
    let G = GeometrySmith(N, V, L, roughness);
    let numerator = D * G * F;
    let denominator = 4.0 * NdotV * NdotL + 0.0001;
    return numerator / denominator;
}

fn specular_response(N: vec3<f32>, V: vec3<f32>, L: vec3<f32>, radiance: vec3<f32>, F0: vec3<f32>, F90: f32, roughness: f32) -> vec3<f32> {
    let H = normalize(L + V);
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0, F90);
    return ggx_specular(N, V, L, H, F, roughness) * radiance * max(dot(N, L), 0.0);
}

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
    F90: f32, 
    metallic: f32, 
    roughness: f32, 
    albedo: vec3<f32>
) -> vec3<f32> {
    let H = normalize(L + V);
    let NdotL = max(dot(N, L), 0.0);
    let F = FresnelSchlick(max(dot(H, V), 0.0), F0, F90);

    let kS = F;
    var kD = vec3<f32>(1.0) - kS;
    kD *= 1.0 - metallic;

    let specular = ggx_specular(N, V, L, H, F, roughness);
    let diffuse = kD * albedo / PI;

    return diffuse * radiance * diffuse_response(dot(N, L)) + specular * radiance * NdotL;
}

// PCF kernel radius in texels.
const PCF_RADIUS: i32 = 2;

// In texels of the cascade sampled, so every cascade gets the same relative bias.
const NORMAL_OFFSET_TEXELS: f32 = 1.5;
const DEPTH_BIAS_TEXELS: f32 = 1.0;

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

    // `Level` sampling: the cascade is picked per fragment, so this runs in non-uniform
    // control flow.
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

// y: fade-out over the last tenth of the shadow distance (0 inside, 1 past it or with
// no cascade).
fn sun_shadow_parts(world_pos: vec3<f32>, N: vec3<f32>) -> vec2<f32> {
    let count = i32(shadow.params.x);
    let distance = shadow.params.y;
    let depth = dot(world_pos - shadow.view_pos.xyz, shadow.view_dir.xyz);
    if (count == 0 || depth >= distance) {
        return vec2<f32>(1.0, 1.0);
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
    return vec2<f32>(lit, fade);
}

fn eval_sh(N: vec3<f32>, object: u32) -> vec3<f32> {
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
        out += objects[object].sh[i].xyz * b[i] * lobe[i];
    }
    return max(out, vec3<f32>(0.0));
}

fn probe_irradiance(world_pos: vec3<f32>, N: vec3<f32>, instance: vec3<u32>) -> vec3<f32> {
    let proxy = proxy_irradiance(world_pos, N, instance.z);
    return select(eval_sh(N, instance.x), proxy.rgb, proxy.w > 0.5);
}

fn flat_ambient(N: vec3<f32>) -> vec3<f32> {
    return ambient_radiance(lighting.ambient, N);
}
