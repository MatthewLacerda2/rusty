// engine/shaders/outline/mask.wgsl — the selection outline's mask (#1175).
//
// Draws a solid the forward pass already drew, from the same mesh, instance,
// per-draw uniform, bone palette and material group, into a one-channel mask:
// 1 where the selection covers the view. Vertices are placed by the forward pass's
// own `vertex_to_world`, so the mask agrees with the solid about where it is, how it
// is posed and which LOD it shows; a surface shader's vertex blocks (#1524) are not
// run (`passes::outline`). `outline/edge.wgsl` rings it.

#import common::VertexInput
#import forward::bindings::{camera, material, normal_to_world, s_diffuse, t_diffuse, vertex_to_world}
#import forward::mapping

struct MaskVertex {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    // Where a world-mapped material reads its albedo (#1573).
    @location(1) world_position: vec3<f32>,
    @location(2) world_normal: vec3<f32>,
};

@vertex
fn vs_mask(model: VertexInput, @builtin(instance_index) instance: u32) -> MaskVertex {
    var out: MaskVertex;
    let to_world = vertex_to_world(model, instance);
    let world = to_world * vec4<f32>(model.position, 1.0);
    out.clip_position = camera.view_proj * world;
    out.tex_coords = model.tex_coords;
    out.world_position = world.xyz;
    out.world_normal = normal_to_world(to_world, model.normal);
    return out;
}

// A cutout material's clipped texels are not the solid, so they stay out of the
// mask, as `fs_main` and `prepass_clip` clip them. Everything else covers it,
// translucent or not.
@fragment
fn fs_mask(in: MaskVertex) -> @location(0) vec4<f32> {
    let m = material();
    if (m.use_cutout == 1u) {
        var alpha = m.color_tint.a;
        if (m.use_texture == 1u) {
            let maps = mapping::coords(in.tex_coords, in.world_position, normalize(in.world_normal));
            alpha *= mapping::sample(t_diffuse, s_diffuse, maps).a;
        }
        if (alpha < m.alpha_cutoff) {
            discard;
        }
    }
    return vec4<f32>(1.0);
}
