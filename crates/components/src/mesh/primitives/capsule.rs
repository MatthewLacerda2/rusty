use glam::Vec3;

use super::fill_tangents_unwrapped;
use crate::mesh::Vertex;

pub fn generate_capsule(
    radius: f32,
    height: f32,
    segments: u32,
    rings: u32,
) -> (Vec<Vertex>, Vec<u32>) {
    let half_tube = (height * 0.5 - radius).max(0.0);
    let rows = capsule_rows(rings, half_tube);
    // Texture V runs down the profile by arc length, so the tube is not stretched.
    let arc = std::f32::consts::PI * radius + 2.0 * half_tube;
    let mut vertices = Vec::new();
    let mut walked = 0.0;
    for (r, &(theta, offset)) in rows.iter().enumerate() {
        if r > 0 {
            let (prev_theta, prev_offset) = rows[r - 1];
            walked += (theta - prev_theta) * radius + (prev_offset - offset);
        }
        let v = 1.0 - walked / arc;
        for s in 0..=segments {
            let phi = (s as f32) * std::f32::consts::TAU / (segments as f32);
            let norm = Vec3::new(
                theta.sin() * phi.cos(),
                theta.cos(),
                theta.sin() * phi.sin(),
            );
            let pos = norm * radius + Vec3::Y * offset;
            vertices.push(Vertex::new(pos, norm, [s as f32 / segments as f32, v]));
        }
    }

    let last = rows.len() as u32 - 2;
    let mut indices = Vec::new();
    for r in 0..=last {
        for s in 0..segments {
            let idx0 = r * (segments + 1) + s;
            let idx2 = idx0 + segments + 1;
            if r != 0 {
                indices.extend_from_slice(&[idx0, idx0 + 1, idx2 + 1]);
            }
            if r != last {
                indices.extend_from_slice(&[idx0, idx2 + 1, idx2]);
            }
        }
    }

    fill_tangents_unwrapped(vertices, indices)
}

fn capsule_rows(rings: u32, half_tube: f32) -> Vec<(f32, f32)> {
    let step = std::f32::consts::FRAC_PI_2 / (rings as f32);
    let top = (0..=rings).map(|r| (r as f32 * step, half_tube));
    let bottom = (0..=rings).map(|r| ((rings + r) as f32 * step, -half_tube));
    top.chain(bottom).collect()
}
