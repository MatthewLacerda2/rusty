// engine/shaders/lighting/screen_space.wgsl — what the screen-space indirect passes
// share: SSAO (`ssao.wgsl`, #436) and SSIL (`postfx/ssil.wgsl`, #1750) rebuild a
// normal from depth the same way, sample the same cosine-weighted golden-angle
// hemisphere under the same per-pixel noise, and blur by the same depth weight.
// Pure functions of their arguments: each pass keeps its own bindings.

#define_import_path lighting::screen_space

// Interleaved gradient noise (Jimenez 2014): a per-pixel rotation a blur removes.
fn ign(px: vec2<f32>) -> f32 {
    return fract(52.9829189 * fract(dot(px, vec2<f32>(0.06711056, 0.00583715))));
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

// How much a tap at view depth `d` counts beside a centre at `center`: taps whose
// depth differs by more than ~5% of the view distance fall away, so a blur never
// bleeds across a silhouette.
fn depth_weight(d: f32, center: f32) -> f32 {
    return exp(-abs(d - center) / (0.05 * max(center, 0.1)));
}
