//! A primitive's indices as the one triangle list every reader walks: strips and fans
//! unrolled, every index naming a vertex, whole triangles only.

use gltf::mesh::Mode;

/// Non-surface modes return `None`: their indices on the triangle path would corrupt
/// geometry.
pub(super) fn triangle_list(
    mode: Mode,
    indices: Option<Vec<u32>>,
    vertex_count: u32,
) -> Option<Vec<u32>> {
    let indices = indices.unwrap_or_else(|| (0..vertex_count).collect());
    match mode {
        Mode::Triangles => Some(indices),
        Mode::TriangleStrip => Some(strip_triangles(&indices)),
        Mode::TriangleFan => Some(fan_triangles(&indices)),
        Mode::Points | Mode::Lines | Mode::LineLoop | Mode::LineStrip => {
            rusty_core::warn_held!(
                "[Asset] skipping glTF primitive: mode {mode:?} is not a triangle surface"
            );
            None
        }
    }
}

fn strip_triangles(strip: &[u32]) -> Vec<u32> {
    strip
        .windows(3)
        .enumerate()
        .flat_map(|(i, w)| {
            if i % 2 == 0 {
                [w[0], w[1], w[2]]
            } else {
                [w[1], w[0], w[2]]
            }
        })
        .collect()
}

fn fan_triangles(fan: &[u32]) -> Vec<u32> {
    let Some((&hub, rim)) = fan.split_first() else {
        return Vec::new();
    };
    rim.windows(2).flat_map(|w| [hub, w[0], w[1]]).collect()
}

/// The readers index vertices by these and walk them in threes, so a primitive naming a
/// vertex it lacks is dropped and a trailing partial triangle cut.
pub(super) fn whole_triangles(
    mut local: Vec<u32>,
    vertex_count: usize,
    file: &str,
    primitive: &gltf::Primitive,
) -> Option<Vec<u32>> {
    let at = primitive.index();
    if let Some(&bad) = local.iter().find(|&&i| i as usize >= vertex_count) {
        rusty_core::warn_held!(
            "[Asset] {file}: skipping glTF primitive {at}: index {bad} names one of {vertex_count} vertices"
        );
        return None;
    }
    if !local.len().is_multiple_of(3) {
        rusty_core::warn_held!(
            "[Asset] {file}: glTF primitive {at}: {} indices are not whole triangles; the last are cut",
            local.len()
        );
        local.truncate(local.len() / 3 * 3);
    }
    Some(local)
}
