// engine/shaders/lighting/screen_space.wgsl — what the screen-space indirect passes
// share: SSAO (`ssao.wgsl`, #436) and SSIL (`postfx/ssil.wgsl`, #1750) read the
// view's depth the same way, rebuild a normal from it the same way, sample the same
// cosine-weighted golden-angle hemisphere under the same per-pixel noise, take the
// same slope-scaled bias, and blur and upsample by the same depth weight.
// Each pass keeps its own bindings: a function takes the depth texture, the texture
// it filters and the pass's camera ([`View`]) as arguments.
// Full-resolution pixel `px` is a depth texel; a pass at a lower resolution reads
// texel `t` as pixel `t * step`.

#define_import_path lighting::screen_space

// The camera a pass reads its depth through.
struct View {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
    // The camera's forward axis (unit).
    forward: vec3<f32>,
};

// Interleaved gradient noise (Jimenez 2014): a per-pixel rotation a blur removes.
fn ign(px: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(px, vec2<f32>(0.06711056, 0.00583715))));
}

// The depth at pixel `px`, clamped to the texture.
fn load_depth(t: texture_depth_2d, px: vec2<i32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(t));
    return textureLoad(t, clamp(px, vec2<i32>(0), dims - 1), 0);
}

// World position of pixel `px` at NDC depth `d`.
fn world_at(t: texture_depth_2d, v: View, px: vec2<i32>, d: f32) -> vec3<f32> {
    let uv = (vec2<f32>(px) + 0.5) / vec2<f32>(textureDimensions(t));
    let w = v.inv_view_proj * vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, d, 1.0);
    return w.xyz / w.w;
}

fn world_px(t: texture_depth_2d, v: View, px: vec2<i32>) -> vec3<f32> {
    return world_at(t, v, px, load_depth(t, px));
}

// Distance along the camera's forward axis, the depth the range checks and blurs
// compare.
fn view_depth(v: View, world: vec3<f32>) -> f32 {
    return dot(world - v.eye, v.forward);
}

// View depth at texel `tex` of a pass `step` pixels to a texel: its pixel's.
fn texel_depth(t: texture_depth_2d, v: View, tex: vec2<i32>, step: i32) -> f32 {
    return view_depth(v, world_px(t, v, tex * step));
}

// Where world point `S` lands on screen, in [0, 1); x < 0 behind the eye or off it.
fn screen_uv(v: View, S: vec3<f32>) -> vec2<f32> {
    let clip = v.view_proj * vec4<f32>(S, 1.0);
    if (clip.w <= 0.0) {
        return vec2<f32>(-1.0);
    }
    let ndc = clip.xy / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if (any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
        return vec2<f32>(-1.0);
    }
    return uv;
}

// Of the two one-sided differences along an axis, the one that stays on this
// surface (smaller depth step along the view axis `forward`), so a silhouette never
// bends the normal.
fn flatter(back: vec3<f32>, fwd: vec3<f32>, forward: vec3<f32>) -> vec3<f32> {
    if (dot(back, back) < 1e-12 || (dot(fwd, fwd) > 1e-12 && abs(dot(fwd, forward)) < abs(dot(back, forward)))) {
        return fwd;
    }
    return back;
}

// The camera-facing normal at pixel `px` (world `P`), rebuilt from the depths `step`
// pixels around it; zero where they are degenerate.
fn normal_at(t: texture_depth_2d, v: View, px: vec2<i32>, P: vec3<f32>, step: i32) -> vec3<f32> {
    let dx = flatter(P - world_px(t, v, px - vec2<i32>(step, 0)), world_px(t, v, px + vec2<i32>(step, 0)) - P, v.forward);
    let dy = flatter(P - world_px(t, v, px - vec2<i32>(0, step)), world_px(t, v, px + vec2<i32>(0, step)) - P, v.forward);
    let c = cross(dx, dy);
    if (dot(c, c) < 1e-20) {
        return vec3<f32>(0.0);
    }
    let N = normalize(c);
    return select(N, -N, dot(N, v.eye - P) < 0.0);
}

// An orthonormal frame around `N`: its tangent and bitangent.
fn tangents(N: vec3<f32>) -> mat2x3<f32> {
    let helper = select(vec3<f32>(1.0, 0.0, 0.0), vec3<f32>(0.0, 1.0, 0.0), abs(N.x) > 0.9);
    let T = normalize(helper - N * dot(helper, N));
    return mat2x3<f32>(T, cross(N, T));
}

// Sample `i` of `n`: a cosine-weighted hemisphere direction around `N` on a
// golden-angle spiral, turned by `rot` radians (`tb`: `tangents(N)`).
fn hemisphere(i: u32, n: u32, rot: f32, N: vec3<f32>, tb: mat2x3<f32>) -> vec3<f32> {
    let t = (f32(i) + 0.5) / f32(n);
    let phi = f32(i) * 2.39996323 + rot;
    let sin_t = sqrt(t);
    return tb[0] * (cos(phi) * sin_t) + tb[1] * (sin(phi) * sin_t) + N * sqrt(1.0 - t);
}

// How many pixels a `radius` offset at `P`, across the view, spans on a `dims`
// screen.
fn radius_in_pixels(v: View, P: vec3<f32>, radius: f32, dims: vec2<f32>) -> f32 {
    let f = v.forward;
    let side = normalize(cross(f, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(f.y) > 0.9)));
    let a = v.view_proj * vec4<f32>(P, 1.0);
    let b = v.view_proj * vec4<f32>(P + side * radius, 1.0);
    return length((b.xy / b.w - a.xy / a.w) * 0.5 * dims);
}

// The slope-scaled bias at surface point `P` with normal `N`, for samples `radius`
// out on a `dims` screen: a sample is snapped to a pixel centre, and at a grazing
// angle one pixel spans a long stretch of the same surface, so allow one pixel's
// worth of depth change along it before calling anything another surface.
fn slope_bias(v: View, P: vec3<f32>, N: vec3<f32>, radius: f32, dims: vec2<f32>) -> f32 {
    let r_px = radius_in_pixels(v, P, radius, dims);
    let grazing = max(abs(dot(N, normalize(v.eye - P))), 0.1);
    return 0.02 * radius + radius / max(r_px, 1e-3) / grazing;
}

// How much a tap at view depth `d` counts beside a centre at `center`: taps whose
// depth differs by more than ~5% of the view distance fall away, so a blur never
// bleeds across a silhouette.
fn depth_weight(d: f32, center: f32) -> f32 {
    return exp(-abs(d - center) / (0.05 * max(center, 0.1)));
}

// A depth-aware box blur of `src` around texel `base`, `reach` texels each way, at a
// pass `step` pixels to a texel (centre view depth `center`).
fn bilateral_blur(t: texture_depth_2d, v: View, src: texture_2d<f32>, base: vec2<i32>, step: i32, reach: i32, center: f32) -> vec4<f32> {
    let last = vec2<i32>(textureDimensions(src)) - 1;
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var y = -reach; y <= reach; y = y + 1) {
        for (var x = -reach; x <= reach; x = x + 1) {
            let tap = clamp(base + vec2<i32>(x, y), vec2<i32>(0), last);
            let w = depth_weight(texel_depth(t, v, tap, step), center);
            sum += textureLoad(src, tap, 0) * w;
            weight += w;
        }
    }
    return sum / max(weight, 1e-4);
}

// Full resolution at pixel `px` (view depth `center`) from `src`, `step` pixels to
// a texel: the four texels around the pixel, each weighted bilinearly and by how
// close its depth is to the pixel's. Where all four sit on other surfaces (a thin
// silhouette), the closest in depth answers alone.
fn bilateral_upsample(t: texture_depth_2d, v: View, src: texture_2d<f32>, px: vec2<i32>, step: i32, center: f32) -> vec4<f32> {
    let f = vec2<f32>(px) / f32(step);
    let t0 = vec2<i32>(floor(f));
    let fr = f - floor(f);
    let last = vec2<i32>(textureDimensions(src)) - 1;
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    var nearest = vec4<f32>(0.0);
    var nearest_gap = 3.4e38;
    for (var i = 0; i < 4; i = i + 1) {
        let o = vec2<i32>(i & 1, i >> 1u);
        let tap = min(t0 + o, last);
        let td = texel_depth(t, v, tap, step);
        let c = textureLoad(src, tap, 0);
        let bw = mix(1.0 - fr, fr, vec2<f32>(o));
        let w = bw.x * bw.y * depth_weight(td, center);
        sum += c * w;
        weight += w;
        let gap = abs(td - center);
        if (gap < nearest_gap) {
            nearest_gap = gap;
            nearest = c;
        }
    }
    if (weight < 1e-3) {
        return nearest;
    }
    return sum / weight;
}
