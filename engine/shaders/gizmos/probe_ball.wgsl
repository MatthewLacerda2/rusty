// the reflection-probe gizmo's mirror ball , Unity's reflection-probe preview sphere: a perfect
// mirror of the probe's raw capture, mip 0, no box projection, no lighting.

#import forward::bindings::{camera, draw, lighting, s_linear_clamp, t_refl_cube}

const UNBAKED_GREY: vec3<f32> = vec3<f32>(0.5, 0.5, 0.5);

struct BallVertex {
    @builtin(position) clip_position: vec4<f32>,
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
    let c_clip = m * vec4<f32>(center, 1.0);
    let c_ndc = c_clip.xy / c_clip.w;
    let near = camera.inv_view_proj * vec4<f32>(c_ndc, 0.0, 1.0);
    let far = camera.inv_view_proj * vec4<f32>(c_ndc, 1.0, 1.0);
    let toward = select(to_eye / d, normalize(near.xyz / near.w - far.xyz / far.w), ortho);
    let helper = select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(toward.y) > 0.99);
    let right = normalize(cross(helper, toward));
    let up = cross(toward, right);
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
    let ndc = in.clip.xy / in.clip.z;
    let near = camera.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let far = camera.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    let origin = near.xyz / near.w;
    let dir = normalize(far.xyz / far.w - origin);
    let oc = origin - in.center;
    let b = dot(oc, dir);
    let h = b * b - (dot(oc, oc) - in.radius * in.radius);
    let t = -b - sqrt(max(h, 0.0));
    if (h < 0.0 || t < 0.0) {
        discard;
    }
    let hit = origin + dir * t;
    let reflected = reflect(dir, normalize(hit - in.center));
    var colour = UNBAKED_GREY;
    if (in.resident > 0u) {
        // Mip 0 of the probe's chain is `mip_base` levels down the array (a smaller cube shares
        // an array of larger ones).
        let p = lighting.refl_probes[in.resident - 1u];
        colour = textureSampleLevel(t_refl_cube, s_linear_clamp, reflected, p.layer, p.mip_base).rgb;
    }
    let at = camera.view_proj * vec4<f32>(hit, 1.0);
    var out: BallFragment;
    out.colour = vec4<f32>(colour, 1.0);
    out.depth = at.z / at.w;
    return out;
}
