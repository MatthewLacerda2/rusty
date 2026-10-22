//! Which way round a placed mesh's triangles face (#1134, #1142). A world matrix
//! with a negative determinant mirrors what it places and reverses its winding, so
//! a pipeline that culls by facing needs a twin that takes clockwise triangles as
//! front. The shadow casters pick theirs here.

use glam::Mat4;

/// Whether `world` mirrors what it places: a negative determinant reverses a
/// mesh's winding, so face culling would keep the wrong side.
pub(crate) fn is_mirrored(world: Mat4) -> bool {
    world.determinant() < 0.0
}

/// The winding a pipeline drawing mirrored (or plain) meshes takes as front.
pub(crate) fn front_face(mirrored: bool) -> wgpu::FrontFace {
    if mirrored {
        wgpu::FrontFace::Cw
    } else {
        wgpu::FrontFace::Ccw
    }
}
