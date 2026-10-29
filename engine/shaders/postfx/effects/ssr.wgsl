// engine/shaders/postfx/effects/ssr.wgsl — real screen-space reflections, on the HDR
// image before bloom (High tier only; Low/Medium rely on the forward shader's cubemap
// reflection). Slot 0: x the mode (`2`: screen-space, else off).

#define_import_path postfx::effects::ssr

#import common::equirect_uv
#import postfx::frame::{Frag, params, t_color, s_color, t_skybox, s_skybox, load_depth}

// Reconstruct a surface normal from neighbouring depth via screen-space
// derivatives — avoids a full normal G-buffer for the SSR pass.
fn normal_from_depth(uv: vec2<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let dx = dpdx(world_pos);
    let dy = dpdy(world_pos);
    return normalize(cross(dx, dy));
}

// March the reflected ray in world space, project each step to screen, and compare
// reconstructed depth. Falls back to the sky on a miss so reflective surfaces never
// go black.
fn screen_space_reflection(uv: vec2<f32>, world_pos: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let v = normalize(world_pos - params.camera_pos.xyz);
    let r = reflect(v, n);
    var p = world_pos + r * 0.1;
    let step = 0.25;
    for (var i = 0; i < 24; i = i + 1) {
        p = p + r * step;
        let clip = params.view_proj * vec4<f32>(p, 1.0);
        if (clip.w <= 0.0) { break; }
        let s = clip.xyz / clip.w;
        let suv = vec2<f32>(s.x * 0.5 + 0.5, 1.0 - (s.y * 0.5 + 0.5));
        if (suv.x < 0.0 || suv.x > 1.0 || suv.y < 0.0 || suv.y > 1.0) { break; }
        let scene_depth = load_depth(suv);
        if (scene_depth < s.z - 0.0005 && s.z - scene_depth < 0.02) {
            // Explicit-LOD sample: a gradient instruction inside this break-bounded
            // (varying-iteration) loop fails to compile under D3D12/FXC.
            return textureSampleLevel(t_color, s_color, suv, 0.0).rgb;
        }
    }
    return sky_reflection(r);
}

fn sky_reflection(r: vec3<f32>) -> vec3<f32> {
    // Level 0, as the skybox pass reads it: the atan2 seam's derivatives would
    // pick a coarse mip (#1322).
    return textureSampleLevel(t_skybox, s_skybox, equirect_uv(r), 0.0).rgb;
}

// Additive, grazing-weighted.
fn hdr(c: vec3<f32>, f: Frag, p: vec4<f32>) -> vec3<f32> {
    // The derivatives are taken before the branch, in uniform control flow: inside
    // it a quad's neighbour on the sky (or with SSR off) is outside the branch, its
    // `world_pos` undefined there, and Metal's normal then changed frame to frame
    // on silhouette pixels (#1525).
    let n = normal_from_depth(f.uv, f.world_pos);
    var out = c;
    if (p.x > 1.5 && f.depth < 1.0) {
        let refl = screen_space_reflection(f.uv, f.world_pos, n);
        let v = normalize(params.camera_pos.xyz - f.world_pos);
        let fresnel = pow(1.0 - max(dot(n, v), 0.0), 3.0);
        out += refl * (0.05 + fresnel * 0.3);
    }
    return out;
}
