// engine/shaders/gizmos/navmesh.wgsl — the NavMesh gizmo (#1910), Unity's baked
// navmesh overlay: one translucent quad per walkable span, already in world space
// (the shell builds the triangles from the navigation graph), flat in the overlay
// draw's tint. `render::draw::overlays::navmesh` draws it in the editor's handles
// pass, through the forward pipeline layout, alpha-blended without writing depth.
//
// The vertex is a bare position (12 bytes, not the 88 of a mesh `Vertex`): a
// Dust2-sized navmesh is hundreds of thousands of quads.

#import forward::bindings::{camera, draw, material}

@vertex
fn vs_navmesh(@location(0) position: vec3<f32>) -> @builtin(position) vec4<f32> {
    return camera.view_proj * draw.model_matrix * vec4<f32>(position, 1.0);
}

@fragment
fn fs_navmesh() -> @location(0) vec4<f32> {
    return material().color_tint;
}
