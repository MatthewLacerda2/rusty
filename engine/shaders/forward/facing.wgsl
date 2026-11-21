
#define_import_path forward::facing

#import forward::bindings::camera

// Whether a material that draws back faces (`two_sided`: render face `Back` or `Both`) is seen
// here from behind, so it shades with its normal flipped, as URP's *Mirror* double-sided normal
// mode does (`facing_normal`, `facing_tangent`).
fn seen_from_behind(n: vec3<f32>, p: vec3<f32>, two_sided: bool) -> bool {
    let face = cross(dpdx(p), dpdy(p));
    let seen = select(-face, face, dot(face, toward_viewer(p)) >= 0.0);
    return two_sided && dot(n, seen) < 0.0;
}

fn toward_viewer(p: vec3<f32>) -> vec3<f32> {
    let vp = camera.view_proj;
    let w_row = vec3<f32>(vp[0].w, vp[1].w, vp[2].w);
    if (dot(w_row, w_row) > 0.0) {
        return camera.camera_pos - p;
    }
    return -vec3<f32>(vp[0].z, vp[1].z, vp[2].z);
}

fn facing_normal(n: vec3<f32>, behind: bool) -> vec3<f32> {
    let unit = normalize(n);
    return select(unit, -unit, behind);
}

// The tangent frame for a normal flipped by `facing_normal`: its handedness `w` turns with it,
// so the bitangent `w * cross(N, T)` keeps its direction and a normal map reads mirrored
// through the surface (`T·x + B·y − N·z`), not as a saddle.
fn facing_tangent(t: vec4<f32>, behind: bool) -> vec4<f32> {
    return vec4<f32>(t.xyz, select(t.w, -t.w, behind));
}
