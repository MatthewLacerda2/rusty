// engine/shaders/outline.wgsl — the editor's selection outline (#1175).
//
// The outline is a second draw of a solid the forward pass already drew: the same
// mesh, instance, per-draw uniform and bone palette, bound at the same slots
// (`render::draw::overlays::outline`). Its vertices are placed by the forward pass's
// own `vertex_to_world`, so the outline can never disagree with the solid about
// where it is, how it is posed, or which LOD it shows.
//
// The hull grows by pushing each vertex along its normal, a fixed number of pixels
// on screen: there is no pivot to grow about (#1165) and the ring is as wide on a
// far crate as on a near one. The outline pipelines cull front faces, so only the
// pushed-out back faces show, as a ring around the silhouette. The normal is the
// mesh's *outline* normal (`render::gpu::mesh::outline_normals`, vertex slot 1):
// averaged over every vertex at one position, so a hard-edged mesh's faces push
// out together. With each face's own normal, a box's faces split apart, and a back
// face pointing away from the camera drags its edge of the ring in behind the box.

#import common::VertexInput
#import forward::bindings::{camera, vertex_to_world}

// The ring's width in pixels.
const OUTLINE_PX: f32 = 3.0;
// A vibrant orange, unlit and unfogged.
const OUTLINE_COLOR: vec4<f32> = vec4<f32>(1.0, 0.5, 0.0, 1.0);

@vertex
fn vs_main(
    model: VertexInput,
    @location(7) outline_normal: vec3<f32>,
    @builtin(instance_index) instance: u32,
) -> @builtin(position) vec4<f32> {
    let to_world = vertex_to_world(model, instance);
    let clip = camera.view_proj * to_world * vec4<f32>(model.position, 1.0);
    let world_normal = (to_world * vec4<f32>(outline_normal, 0.0)).xyz;
    let clip_normal = camera.view_proj * vec4<f32>(world_normal, 0.0);
    // Which way the normal points on screen, in pixels: the derivative of the
    // projected position `xy / w` along the normal, scaled to the view.
    let on_screen = (clip_normal.xy * clip.w - clip.xy * clip_normal.w) * camera.viewport.xy;
    let length_px = length(on_screen);
    if (length_px < 1e-6) {
        // Facing the camera head-on: there is no screen direction to grow along.
        return clip;
    }
    // `OUTLINE_PX` pixels along it in NDC (two units across the view), times `w`.
    let ndc = on_screen / length_px * OUTLINE_PX * 2.0 * camera.viewport.zw;
    return vec4<f32>(clip.xy + ndc * clip.w, clip.zw);
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return OUTLINE_COLOR;
}
