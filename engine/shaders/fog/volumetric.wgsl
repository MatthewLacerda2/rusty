// engine/shaders/fog/volumetric.wgsl — reading the volumetric fog (#1703): where a
// point falls in the froxel grid, and the in-scattered light and transmittance the
// integrated volume holds between the eye and it. The grid itself is built by
// `volumetric_fog.wgsl` (`render::postfx::effects::volumetric_fog`); every pass that
// fogs (the forward and transparent surfaces, decals folded into them, the sky and
// particles) reads it through here, so they agree.
//
// The camera uniform's `volumetric` is x 1 when this camera reads the volume (0: a
// uniform branch, no fetch), y the depth slices, z the far end (its *Volumetric Fog
// Distance*) and w the near end of the exponential slices.

#define_import_path fog::volumetric

// The in-scatter (rgb) and transmittance (a) of no fog at all.
const NO_VOLUMETRIC: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);

// Where view depth `depth` falls among the slices, in slice-boundary units: 0 at the
// eye, 1 at the first slice's far side (linear up to it), then exponential from
// `near` to `far` at `slices`. Slice k spans [k, k+1].
fn froxel_slice(vol: vec4<f32>, depth: f32) -> f32 {
    let n = vol.y;
    let ratio = log(vol.z / vol.w);
    let first = vol.w * exp(ratio / n);
    if (depth <= first) {
        return max(depth, 0.0) / first;
    }
    return n * log(depth / vol.w) / ratio;
}

// The view depth at slice coordinate `s`: [`froxel_slice`]'s inverse.
fn froxel_depth(vol: vec4<f32>, s: f32) -> f32 {
    let n = vol.y;
    let ratio = log(vol.z / vol.w);
    if (s <= 1.0) {
        return s * vol.w * exp(ratio / n);
    }
    return vol.w * exp(ratio * s / n);
}

// The integrated volume at screen `uv` and slice coordinate `s`: texel k holds the
// light gathered from the eye to boundary k + 1, so the first slice's stretch fades
// in from no fog; past the far end it is the far end's.
fn volume_at(t: texture_3d<f32>, smp: sampler, vol: vec4<f32>, uv: vec2<f32>, s: f32) -> vec4<f32> {
    let w = (s - 0.5) / vol.y;
    let fetched = textureSampleLevel(t, smp, vec3<f32>(uv, w), 0.0);
    return mix(NO_VOLUMETRIC, fetched, clamp(s, 0.0, 1.0));
}

// The volumetric fog between the eye and `world`, as `view_proj` and the camera's
// `view_z` plane (`depth = dot(xyz, p) + w`) see it: rgb the light it scatters
// toward the eye, a how much of `world`'s own light gets through.
fn volumetric_fog(
    t: texture_3d<f32>,
    smp: sampler,
    vol: vec4<f32>,
    view_proj: mat4x4<f32>,
    view_z: vec4<f32>,
    world: vec3<f32>,
) -> vec4<f32> {
    if (vol.x < 0.5) {
        return NO_VOLUMETRIC;
    }
    let clip = view_proj * vec4<f32>(world, 1.0);
    let ndc = clip.xy / max(clip.w, 1e-6);
    let uv = clamp(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    let depth = dot(view_z.xyz, world) + view_z.w;
    return volume_at(t, smp, vol, uv, froxel_slice(vol, depth));
}

// The sky's volumetric fog at clip-space position `clip` (the sky fragment's own,
// interpolated from its vertices): the whole volume, as for any point past its far
// end. The fragment's screen position, not a point along its view direction, so an
// orthographic camera's sky reads each column of the grid too.
fn volumetric_sky(t: texture_3d<f32>, smp: sampler, vol: vec4<f32>, clip: vec4<f32>) -> vec4<f32> {
    if (vol.x < 0.5) {
        return NO_VOLUMETRIC;
    }
    let ndc = clip.xy / max(clip.w, 1e-6);
    let uv = clamp(vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5), vec2<f32>(0.0), vec2<f32>(1.0));
    return volume_at(t, smp, vol, uv, vol.y + 1.0);
}

// `color` seen through `fog` (`volumetric_fog`'s result): dimmed, then lit by the air.
fn through_volume(color: vec3<f32>, fog: vec4<f32>) -> vec3<f32> {
    return color * fog.a + fog.rgb;
}
