// engine/shaders/forward/facing.wgsl — which side of a two-sided face the viewer
// sees, and the normal and tangent frame it is shaded with (#1615). `surface_color`
// in `shader.wgsl` reads them first, before any per-pixel branch.

#define_import_path forward::facing

#import forward::bindings::camera

// Whether a material that draws back faces (`two_sided`: render face `Back` or
// `Both`, #1615) is seen here from behind, so it shades with its normal flipped, as
// URP's *Mirror* double-sided normal mode does (`facing_normal`, `facing_tangent`).
// Which side is seen is read off the triangle's own plane on screen (its position's
// derivatives, so the caller runs this before any per-pixel branch), not
// `@builtin(front_facing)`: a mirrored mesh (#1134) draws through the pipeline that
// keeps its clockwise triangles, where `front_facing` would name the wrong side, and
// this needs no twin pipeline for it. `n` is the interpolated vertex normal.
fn seen_from_behind(n: vec3<f32>, p: vec3<f32>, two_sided: bool) -> bool {
    let face = cross(dpdx(p), dpdy(p));
    let seen = select(-face, face, dot(face, toward_viewer(p)) >= 0.0);
    return two_sided && dot(n, seen) < 0.0;
}

// The direction from `p` toward the viewer: to the eye through a perspective lens,
// against the view's forward axis through an orthographic one (#430), whose rays are
// parallel. `view_proj`'s w row is zero only for an orthographic lens, and its z row
// (depth, 0 near to 1 far) points along the view's forward axis for either.
fn toward_viewer(p: vec3<f32>) -> vec3<f32> {
    let vp = camera.view_proj;
    let w_row = vec3<f32>(vp[0].w, vp[1].w, vp[2].w);
    if (dot(w_row, w_row) > 0.0) {
        return camera.camera_pos - p;
    }
    return -vec3<f32>(vp[0].z, vp[1].z, vp[2].z);
}

// `n` normalized, and flipped when `behind` (`seen_from_behind`).
fn facing_normal(n: vec3<f32>, behind: bool) -> vec3<f32> {
    let unit = normalize(n);
    return select(unit, -unit, behind);
}

// The tangent frame for a normal flipped by `facing_normal`: its handedness `w` turns
// with it, so the bitangent `w * cross(N, T)` keeps its direction and a normal map
// reads mirrored through the surface (`T·x + B·y − N·z`), not as a saddle.
fn facing_tangent(t: vec4<f32>, behind: bool) -> vec4<f32> {
    return vec4<f32>(t.xyz, select(t.w, -t.w, behind));
}
