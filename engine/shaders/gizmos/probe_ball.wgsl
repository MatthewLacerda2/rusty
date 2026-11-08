// engine/shaders/gizmos/probe_ball.wgsl — the reflection-probe gizmo's mirror ball
// (#1911), Unity's reflection-probe preview sphere: a perfect mirror of the probe's raw
// capture, mip 0, no box projection, no lighting. `render::draw::overlays::probe_ball`
// draws it in the editor's handles pass, through the forward pipeline layout.
//
// The ball is a true sphere, not a mesh: each one is a quad facing back along the
// view ray through its centre, sized to cover the sphere's silhouette, and each
// fragment casts the camera's ray at the sphere, discards a miss and writes the
// hit's depth, so the ball is round at any distance and depth-tests against the
// scene and the other gizmos as a solid does.
//
// Its placement is the overlay draw's (`draw.model_matrix`: the probe's position, a
// uniform scale of the ball's radius); the instance index is the probe's resident
// slot in `lighting.refl_probes` plus one, 0 for a probe whose cube is not resident
// (unbaked, failed to load, or past the resident count), which draws flat grey.

#import forward::bindings::{camera, draw, lighting, s_refl_cube, t_refl_cube}

// What a probe with no resident cube shows: flat neutral grey, so it reads as unbaked.
const UNBAKED_GREY: vec3<f32> = vec3<f32>(0.5, 0.5, 0.5);

struct BallVertex {
    @builtin(position) clip_position: vec4<f32>,
    // The vertex's clip position again (xyw), for the fragment's ray.
    @location(0) clip: vec3<f32>,
    @location(1) @interpolate(flat) center: vec3<f32>,
    @location(2) @interpolate(flat) radius: f32,
    @location(3) @interpolate(flat) resident: u32,
};

struct BallFragment {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@vertex
fn vs_ball(@builtin(vertex_index) corner: u32, @builtin(instance_index) resident: u32) -> BallVertex {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let center = draw.model_matrix[3].xyz;
    let radius = length(draw.model_matrix[0].xyz);
    // An orthographic projection leaves clip w at 1: view_proj's bottom row is (0,0,0,1).
    let m = camera.view_proj;
    let ortho = m[0].w == 0.0 && m[1].w == 0.0 && m[2].w == 0.0;
    let to_eye = camera.camera_pos - center;
    let d = length(to_eye);
    // The quad faces back along the camera's ray through the centre. In perspective that
    // is toward the eye; in orthographic every ray is parallel, so it is the ray through
    // the centre's NDC, unprojected at depth 0 and 1 (facing the eye there would clip an
    // off-axis ball's silhouette).
    let c_clip = m * vec4<f32>(center, 1.0);
    let c_ndc = c_clip.xy / c_clip.w;
    let near = camera.inv_view_proj * vec4<f32>(c_ndc, 0.0, 1.0);
    let far = camera.inv_view_proj * vec4<f32>(c_ndc, 1.0, 1.0);
    let toward = select(to_eye / d, normalize(near.xyz / near.w - far.xyz / far.w), ortho);
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(toward.y) > 0.99);
    let right = normalize(cross(helper, toward));
    let up = cross(toward, right);
    // The silhouette's radius on the plane through the centre facing the ray: the
    // ball's own in orthographic, the cone's in perspective. An eye inside (or on) the
    // ball collapses the quad to the centre (the ball is not drawn), before the basis,
    // which is NaN for an eye exactly on the probe, can reach the position.
    let cone = radius * d / sqrt(max(d * d - radius * radius, 1e-8));
    let inside = !ortho && d <= radius;
    let c = corners[corner];
    let world = select(center + (right * c.x + up * c.y) * select(cone, radius, ortho), center, inside);
    var out: BallVertex;
    out.clip_position = camera.view_proj * vec4<f32>(world, 1.0);
    out.clip = out.clip_position.xyw;
    out.center = center;
    out.radius = radius;
    out.resident = resident;
    return out;
}

@fragment
fn fs_ball(in: BallVertex) -> BallFragment {
    // The camera's ray through this pixel, unprojected, so a perspective and an
    // orthographic camera both cast it right.
    let ndc = in.clip.xy / in.clip.z;
    let near = camera.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far = camera.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let origin = near.xyz / near.w;
    let dir = normalize(far.xyz / far.w - origin);
    let oc = origin - in.center;
    let b = dot(oc, dir);
    let h = b * b - (dot(oc, oc) - in.radius * in.radius);
    // A miss, or a near hit behind the near plane (an eye just outside the ball): the
    // near plane clips the ball as it clips a solid.
    let t = -b - sqrt(max(h, 0.0));
    if (h < 0.0 || t < 0.0) {
        discard;
    }
    let hit = origin + dir * t;
    let reflected = reflect(dir, normalize(hit - in.center));
    var colour = UNBAKED_GREY;
    if (in.resident > 0u) {
        // Mip 0 of the probe's chain is `mip_base` levels down the array (a smaller
        // cube shares an array of larger ones, #1584).
        let p = lighting.refl_probes[in.resident - 1u];
        colour = textureSampleLevel(t_refl_cube, s_refl_cube, reflected, p.layer, p.mip_base).rgb;
    }
    let at = camera.view_proj * vec4<f32>(hit, 1.0);
    var out: BallFragment;
    out.colour = vec4<f32>(colour, 1.0);
    out.depth = at.z / at.w;
    return out;
}
