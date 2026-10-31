// engine/shaders/common.wgsl — shared GPU struct definitions and helpers imported by
// the forward, shadow, skybox, particle and post-FX passes via `#import common::{…}`.
// Every name here is part of what a baked shader may import, so none ever moves out;
// a helper only common itself calls may live in a module of its own (`sky/noise.wgsl`).

#import lighting::ambient::AmbientLight
#import lighting::reflection_probe::ReflectionProbe
#import sky::noise::sky_fbm

// The scene's distance + height fog (#437). Mirrors the Rust `FogUniform`
// byte-for-byte (naga_oil rejects imported names ending `_<digit>`, hence `_pad_a`).
// `mode`: 0 off, 1 linear, 2 exponential, 3 exponential².
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

// How a point finds its light cluster (#434). Mirrors the Rust `ClusterUniform`;
// built per camera by `render::clusters::ClusterGrid`, which bins on the CPU with
// the same tiles and slices `cluster_index` reads here.
struct Clusters {
    // depth = dot(view_z.xyz, p) + view_z.w: distance along the view axis.
    view_z: vec4<f32>,
    // x scale, y bias, z 1 = log2 slices (perspective) / 0 = linear, w near plane.
    slices: vec4<f32>,
    // Tiles across, tiles up, depth slices; w unused.
    dims: vec4<u32>,
};

struct CameraUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    // The sim's game time in seconds (#398): scaled by the time scale, frozen while
    // paused, 0 in edit mode and previews. Fills vec3 padding, so no layout change.
    time: f32,
    // Rides with the camera so every pass that already binds it fogs for free.
    fog: Fog,
    // This camera's light-cluster grid (#434).
    clusters: Clusters,
    // This camera's view-projection last frame (#1435), what motion vectors reproject
    // through; equal to `view_proj` on a camera's first frame and after a cut.
    prev_view_proj: mat4x4<f32>,
    // x: this camera's index in its stack, bottom to top (#1436), what the velocity
    // target's `.a` carries so motion blur orders a higher camera in front. y: last
    // frame's game time (#1524), what the motion vectors move vertex blocks with (this
    // frame's `time` where the view has no last frame). zw: the march a `receive_ssr`
    // translucent traces (#1570), its step count and step length; zero where the
    // camera runs no SSR (`render::postfx::effects::ssr::transparent`).
    stack: vec4<f32>,
    // `view_proj`'s inverse (#1523): a transparent surface rebuilds the world point
    // of the opaque depth behind it from it, for refraction and absorption.
    inv_view_proj: mat4x4<f32>,
};

// The forward lighting uniform (group 0, binding 1) and its light records. Shared
// so every pass that shades against the scene lights — the forward pass and lit
// particles (#440) — reads the one layout. Mirrors the Rust `LightingUniform`.
struct DirectionalLight {
    direction: vec3<f32>,
    color: vec3<f32>,
    intensity: f32,
    // 1.0 for a `Baked` light (#438): a lightmapped surface skips its direct light.
    baked: f32,
};

struct LightingUniforms {
    ambient: AmbientLight,
    // The procedural sky's settings (#1620), right after `ambient` so the sky pass
    // reads both through a prefix of this struct.
    sky: ProceduralSky,
    // Up to four suns (#434); slot 0 is the one casting the cascaded shadows.
    dir_lights: array<DirectionalLight, 4>,
    num_dir_lights: u32,
    // 1.0 when a skybox panorama is bound at binding 2 (#718); 0.0 means the sky on
    // screen is the procedural gradient, which the env reflection then reflects too.
    sky_textured: f32,
    _sky_pad_a: f32,
    _sky_pad_b: f32,
    // The resident reflection probes (#1206); a draw's instance names the two it
    // blends (`InstanceData.reflection_probe`, packed indices + 1 and weights, #1584). Length is the Rust
    // `MAX_REFLECTION_PROBES`. Mirrors the Rust `LightingUniform` byte-for-byte.
    refl_probes: array<ReflectionProbe, 32>,
};

// One point or spot light of the frame (#434), in the storage array at group 0
// binding 7 (group 3 in the particle pass). Mirrors the Rust `LocalLight`.
struct LocalLight {
    position: vec3<f32>,
    range: f32,
    color: vec3<f32>,
    intensity: f32,
    direction: vec3<f32>,
    kind: u32,        // 0 point, 1 spot
    inner_cone: f32,  // cosine of the inner half-angle
    outer_cone: f32,  // cosine of the outer half-angle
    // First tile in the shadow atlas (#468): a spotlight's one, a point light's six
    // cube faces (+X, -X, +Y, -Y, +Z, -Z); NO_SHADOW when it casts none.
    shadow: u32,
    // 1.0 for a `Baked` light (#438): a lightmapped surface skips its direct light.
    baked: f32,
};

// The light cluster `world` falls in: its screen tile (from `view_proj`) and its
// depth slice. A point off screen clamps to the edge tile. Index order is
// x + tiles_x * (y + tiles_y * slice), as the CPU binner writes it.
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

// The radiance a local light delivers at `world`: inverse-square falloff cut at its
// range, and for a spotlight the smoothed cone. Zero when out of reach.
fn local_light_radiance(light: LocalLight, world: vec3<f32>) -> vec3<f32> {
    let d = distance(light.position, world);
    if (d > light.range) {
        return vec3<f32>(0.0);
    }
    var cone = 1.0;
    if (light.kind == 1u) {
        let theta = dot((light.position - world) / max(d, 1e-6), -light.direction);
        cone = clamp((theta - light.outer_cone) / max(light.inner_cone - light.outer_cone, 1e-4), 0.0, 1.0);
    }
    return light.color * light.intensity * cone / (d * d + 1.0);
}

// ---- Surface decals (#638) ----
// One decal of the frame, in the storage array at group 0 binding 10. Mirrors the
// Rust `GpuDecal` byte-for-byte. Its box is the unit cube in decal space.
struct Decal {
    world_to_decal: mat4x4<f32>,
    right: vec4<f32>,    // xyz the box's right axis (texture u), w the albedo weight
    up: vec4<f32>,       // xyz its up axis (texture -v), w the normal weight
    forward: vec4<f32>,  // xyz the axis out of the surface, w cos(angle fade start)
    color: vec4<f32>,    // albedo tint, a coverage
    surface: vec4<f32>,  // metallic, roughness, metallic weight, roughness weight
    fade: vec4<f32>,     // x occlusion added, y cos(angle fade end)
    layers: vec4<u32>,   // atlas layers: albedo, normal, metallic, roughness
};

// `Decal.layers` for a map the decal does not have.
const NO_LAYER: u32 = 0xffffffffu;

// The material inputs a decal changes, before any light is summed.
struct DecalSurface {
    albedo: vec3<f32>,
    normal: vec3<f32>,
    metallic: f32,
    roughness: f32,
    occlusion: f32,
};

// Fold decal `d` into surface `s` at `world`. `facing` is the surface's geometric
// normal (for the angle fade); `dx`/`dy` are the screen derivatives of `world`,
// taken in uniform control flow, which give every map sample its gradients from the
// decal's own UVs (no smallest-mip fringe where the decal crosses a silhouette).
// Albedo is read through the atlas's sRGB view, the other maps raw.
fn apply_decal(
    s: DecalSurface, d: Decal, world: vec3<f32>, facing: vec3<f32>,
    dx: vec3<f32>, dy: vec3<f32>,
    t_color: texture_2d_array<f32>, t_data: texture_2d_array<f32>, samp: sampler,
) -> DecalSurface {
    let local = (d.world_to_decal * vec4<f32>(world, 1.0)).xyz;
    if (any(abs(local) > vec3<f32>(0.5))) {
        return s;
    }
    // Whole while the surface faces the projector within the fade angle, gone a
    // little past it, and gone near the box's caps.
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
        // The decal's tangent frame laid onto the surface: its right axis flattened
        // into the surface plane, so the bend follows a curved receiver.
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

// `LocalLight.shadow` of a light with no tile in the shadow atlas (#468).
const NO_SHADOW: u32 = 0xffffffffu;

// One tile of the point/spot shadow atlas (#468): a light face's view-projection,
// its square in atlas UVs (xy corner, zw size), and in `params.x` the world size of
// one of its texels a metre from the light. Mirrors the Rust `ShadowTile`.
struct ShadowTile {
    view_proj: mat4x4<f32>,
    rect: vec4<f32>,
    params: vec4<f32>,
};

// The atlas tile holding `light`'s shadow at `world`: a spotlight's one, or the
// point light's cube face along the major axis from the light; NO_SHADOW if none.
fn local_shadow_tile(light: LocalLight, world: vec3<f32>) -> u32 {
    if (light.shadow == NO_SHADOW || light.kind == 1u) {
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

// How lit `world` (normal `N`) is by the light at `light_pos`, through its atlas
// `tile`: 3x3 PCF of comparison taps, kept inside the tile so a kernel never reads
// a neighbour. The surface is pushed off along its normal by a texel or two (more
// at grazing light), sized by the texel footprint at its distance from the light.
// 1.0 outside the tile's view.
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

// Standard mesh vertex layout — position, normal, UVs, skeletal animation data
// (four joint indices + blend weights; no joint cap, #455), and the tangent basis for normal
// mapping (`xyz` unit tangent, `w` handedness sign), then the lightmap UV.
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coords: vec2<f32>,
    @location(3) joint_indices: vec4<u32>,
    @location(4) joint_weights: vec4<f32>,
    @location(5) tangent: vec4<f32>,
    // The lightmap UV (#438): glTF TEXCOORD_1; zeros on a mesh without one.
    @location(6) lightmap_uv: vec2<f32>,
};

// A vertex's skinning matrix (#599): its four joint matrices blended by `weights`.
// Zero weights (a vertex no joint moves) skin as identity. The one skinning function
// the forward and shadow passes share, so a shadow always follows the drawn pose;
// each pass looks the four matrices up in its own copy of the frame's joint array.
fn blend_joints(
    j0: mat4x4<f32>,
    j1: mat4x4<f32>,
    j2: mat4x4<f32>,
    j3: mat4x4<f32>,
    weights: vec4<f32>,
) -> mat4x4<f32> {
    if (weights.x + weights.y + weights.z + weights.w < 0.01) {
        return mat4x4<f32>(
            vec4<f32>(1.0, 0.0, 0.0, 0.0),
            vec4<f32>(0.0, 1.0, 0.0, 0.0),
            vec4<f32>(0.0, 0.0, 1.0, 0.0),
            vec4<f32>(0.0, 0.0, 0.0, 1.0)
        );
    }
    return j0 * weights.x + j1 * weights.y + j2 * weights.z + j3 * weights.w;
}

// How far the sky counts as, for fog: past any level's far wall, so the sky is at
// least as fogged as the farthest geometry in front of it.
const FOG_SKY_DISTANCE: f32 = 1000.0;

// Mean relative fog thickness along a ray whose heights run `a`..`b` above the base
// height: full (1) below it, `e^(-k·h)` above it. Exact integral of that profile,
// so a ray that dips under the base picks up the full layer for that stretch.
fn fog_height_mean(k: f32, a: f32, b: f32) -> f32 {
    let lo = min(a, b);
    let hi = max(a, b);
    if (k <= 0.0) {
        return 1.0;
    }
    if (hi - lo < 1e-3) {
        return exp(-k * max(lo, 0.0));
    }
    let below = min(hi, 0.0) - min(lo, 0.0);
    let above = (exp(-k * max(lo, 0.0)) - exp(-k * max(hi, 0.0))) / k;
    return (below + above) / (hi - lo);
}

// How fogged (0 clear .. 1 fully fog colour) a point is, seen from `eye`. The one
// fog formula every pass uses, so surfaces, particles, decals and the sky agree.
fn fog_factor(fog: Fog, world_pos: vec3<f32>, eye: vec3<f32>) -> f32 {
    if (fog.mode == 0u) {
        return 0.0;
    }
    let d = max(distance(world_pos, eye) - fog.start, 0.0);
    let h = fog_height_mean(fog.height_falloff, eye.y - fog.base_height, world_pos.y - fog.base_height);
    var f: f32;
    if (fog.mode == 1u) {
        f = d * h / max(fog.end - fog.start, 1e-3);
    } else if (fog.mode == 2u) {
        f = 1.0 - exp(-fog.density * d * h);
    } else {
        let x = fog.density * d * h;
        f = 1.0 - exp(-x * x);
    }
    return clamp(f, 0.0, 1.0);
}

// Fade a lit colour into the fog colour — opaque, transparent and alpha-blended
// surfaces (the caller keeps its alpha).
fn apply_fog(fog: Fog, color: vec3<f32>, world_pos: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    return mix(color, fog.color, fog_factor(fog, world_pos, eye));
}

// The sky's fog along view direction `dir`: as fogged as a point at
// FOG_SKY_DISTANCE, faded out above the horizon so the zenith stays sky.
fn sky_fog(fog: Fog, color: vec3<f32>, dir: vec3<f32>, eye: vec3<f32>) -> vec3<f32> {
    let f = fog_factor(fog, eye + dir * FOG_SKY_DISTANCE, eye);
    let horizon = 1.0 - smoothstep(0.0, 0.4, dir.y);
    return mix(color, fog.color, f * horizon);
}

// --- The panorama lookup (#995) -----------------------------------------------
const PI: f32 = 3.14159265359;

// Where direction `dir` lands on an equirectangular panorama: longitude to u (the
// seam at φ = ±π, along -x), polar angle from +y to v. ONE function, three callers
// (the skybox pass, the sky reflection cube's bake and post-FX's SSR fallback), so
// what a surface reflects is the sky on screen. `y` is clamped: a normalised vector
// can round past 1, and `acos` of that is NaN.
fn equirect_uv(dir: vec3<f32>) -> vec2<f32> {
    let phi = atan2(dir.z, dir.x);
    let theta = acos(clamp(dir.y, -1.0, 1.0));
    return vec2<f32>((phi + PI) / (2.0 * PI), theta / PI);
}

// --- The procedural sky (#256) ------------------------------------------------
// The scene's procedural sky settings (#1620, `scene::SkySettings`). Mirrors the
// Rust `ProceduralSkyUniform`. A colour's `w` is 1 when the scene sets it; 0 derives
// it from the ambient colour as the sky always has.
struct ProceduralSky {
    zenith: vec4<f32>,
    horizon: vec4<f32>,
    ground: vec4<f32>,
    // x: cloud coverage (0 clear, 0.5 the default layer, 1 overcast), y: opacity.
    clouds: vec4<f32>,
};

// The colour of the procedural sky along `dir`, before fog: a ground -> horizon ->
// zenith gradient plus a cheap, static cloud layer, shaped by `sky` and tinted from
// `ambient` (the scene's ambient colour) wherever `sky` leaves a colour unset. ONE
// function, two callers: the sky pass draws it when no panorama is bound
// (`sky_gradient.wgsl`), and the sky reflection cube bakes it for the forward pass
// to reflect in the same case (#718, #1553), so a metal surface mirrors the sky that
// is actually on screen. It must stay **affine in `ambient`** for a fixed `sky`
// (`a * ambient + b`, `a` one scalar per direction): every colour is either set (a
// constant) or a scalar multiple of the zenith plus a constant, so it holds, and the
// cube bakes the two terms once per `sky` and applies the ambient colour per sample
// (`sky/prefilter.wgsl`). Its clouds are `sky_fbm` (`sky/noise.wgsl`).

// How far `coverage` shifts the FBM before the cloud mask's ramp (0.45..0.80) carves
// it: exactly 0 at the default 0.5 (the layer the sky always drew), -0.55 at 0 so
// even the FBM's maximum (31/32) stays under the ramp and nothing is carved, +0.8 at
// 1 so even its minimum (0) clears it and every direction is cloud.
fn cloud_shift(coverage: f32) -> f32 {
    let c = coverage - 0.5;
    return c * select(1.1, 1.6, c > 0.0);
}

fn procedural_sky(sky: ProceduralSky, ambient: vec3<f32>, dir: vec3<f32>) -> vec3<f32> {
    // Unset, the zenith is the ambient colour, the horizon a desaturated lift toward
    // white (the classic default-skybox glow), and the ground the darker quarter of
    // the zenith, as the forward pass's hemisphere ambient.
    let zenith = select(ambient, sky.zenith.rgb, sky.zenith.w > 0.5);
    let horizon_color = select(mix(zenith, vec3<f32>(1.0), 0.6), sky.horizon.rgb, sky.horizon.w > 0.5);
    let ground_color = select(zenith * 0.25, sky.ground.rgb, sky.ground.w > 0.5);

    // Blend ground -> horizon below the horizon line, horizon -> zenith above it. The
    // `pow` tightens each band so the horizon glow stays near y = 0.
    let up = clamp(dir.y, 0.0, 1.0);
    let down = clamp(-dir.y, 0.0, 1.0);
    var color = mix(horizon_color, zenith, pow(up, 0.5));
    color = mix(color, ground_color, pow(down, 0.35));

    // Cloud layer, sky hemisphere only. Project the view ray onto a flat sky dome so
    // cells bunch up toward the horizon, then carve soft cloud shapes out of the FBM
    // and tint them a hair brighter than the zenith. Coverage fades out near the
    // horizon (so clouds don't bleed into the glow) and is absent below it.
    if (dir.y > 0.0) {
        let dome = dir.xz / max(dir.y, 0.15);
        let fbm = sky_fbm(dome * 1.6) + cloud_shift(sky.clouds.x);
        let coverage = smoothstep(0.45, 0.80, fbm);
        let band = smoothstep(0.05, 0.35, up);
        let cloud_color = mix(zenith, vec3<f32>(1.0), 0.85);
        color = mix(color, cloud_color, coverage * band * sky.clouds.y);
    }
    return color;
}
