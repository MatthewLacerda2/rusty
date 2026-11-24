// SSR's *Mixed* for the translucents, after `ssr_transparent.wgsl`'s `fs_ray`: one ray serves
// a block of pixels and no resolve averages them, as SSR's does an opaque pixel's, so a ray
// that alone of its neighbours reaches something bright (a lamp, through one ripple) would be
// a dot. `fs_bound` keeps a ray no brighter than the brightest of its eight neighbours' rays,
// and drops one none of them confirms; the forward pass reads what it writes.

#import fullscreen

// rgb radiance, a weight (0: none), as `fs_ray` wrote it.
@group(0) @binding(0) var t_ray: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn fs_bound(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let q = vec2<i32>(pos.xy);
    let ray = textureLoad(t_ray, q, 0);
    if (ray.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let size = vec2<i32>(textureDimensions(t_ray));
    var bound = 0.0;
    var confirmed = false;
    for (var n = 0; n < 9; n = n + 1) {
        let p = q + vec2<i32>(n % 3, n / 3) - 1;
        if (n == 4 || any(p < vec2<i32>(0)) || any(p >= size)) {
            continue;
        }
        let near = textureLoad(t_ray, p, 0);
        if (near.a > 0.0) {
            bound = max(bound, luminance(near.rgb));
            confirmed = true;
        }
    }
    if (!confirmed) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(ray.rgb * min(bound / max(luminance(ray.rgb), 1e-6), 1.0), ray.a);
}
