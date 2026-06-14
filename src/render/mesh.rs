use glam::Vec3;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coords: [f32; 2],
    pub joint_indices: [u32; 4],
    pub joint_weights: [f32; 4],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 5] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position
        1 => Float32x3,  // normal
        2 => Float32x2,  // tex_coords
        3 => Uint32x4,   // joint_indices
        4 => Float32x4,  // joint_weights
    ];

    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }

    pub fn new(pos: Vec3, norm: Vec3, uv: [f32; 2]) -> Self {
        Self {
            position: pos.to_array(),
            normal: norm.to_array(),
            tex_coords: uv,
            joint_indices: [0, 0, 0, 0],
            joint_weights: [1.0, 0.0, 0.0, 0.0],
        }
    }
}

/// Generates a 3D box centered at the origin
pub fn generate_box(width: f32, height: f32, depth: f32) -> (Vec<Vertex>, Vec<u32>) {
    let w = width / 2.0;
    let h = height / 2.0;
    let d = depth / 2.0;

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // 6 faces of a cube (4 vertices per face)
    let faces = [
        // Front face (+Z)
        (
            Vec3::new(-w, -h, d),
            Vec3::new(w, -h, d),
            Vec3::new(w, h, d),
            Vec3::new(-w, h, d),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        // Back face (-Z)
        (
            Vec3::new(w, -h, -d),
            Vec3::new(-w, -h, -d),
            Vec3::new(-w, h, -d),
            Vec3::new(w, h, -d),
            Vec3::new(0.0, 0.0, -1.0),
        ),
        // Right face (+X)
        (
            Vec3::new(w, -h, d),
            Vec3::new(w, -h, -d),
            Vec3::new(w, h, -d),
            Vec3::new(w, h, d),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        // Left face (-X)
        (
            Vec3::new(-w, -h, -d),
            Vec3::new(-w, -h, d),
            Vec3::new(-w, h, d),
            Vec3::new(-w, h, -d),
            Vec3::new(-1.0, 0.0, 0.0),
        ),
        // Top face (+Y)
        (
            Vec3::new(-w, h, d),
            Vec3::new(w, h, d),
            Vec3::new(w, h, -d),
            Vec3::new(-w, h, -d),
            Vec3::new(0.0, 1.0, 0.0),
        ),
        // Bottom face (-Y)
        (
            Vec3::new(-w, -h, -d),
            Vec3::new(w, -h, -d),
            Vec3::new(w, -h, d),
            Vec3::new(-w, -h, d),
            Vec3::new(0.0, -1.0, 0.0),
        ),
    ];

    let uv_coords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

    for (i, &(p0, p1, p2, p3, norm)) in faces.iter().enumerate() {
        let base_idx = (i * 4) as u32;
        vertices.push(Vertex::new(p0, norm, uv_coords[0]));
        vertices.push(Vertex::new(p1, norm, uv_coords[1]));
        vertices.push(Vertex::new(p2, norm, uv_coords[2]));
        vertices.push(Vertex::new(p3, norm, uv_coords[3]));

        // Triangle 1
        indices.push(base_idx);
        indices.push(base_idx + 1);
        indices.push(base_idx + 2);
        // Triangle 2
        indices.push(base_idx);
        indices.push(base_idx + 2);
        indices.push(base_idx + 3);
    }

    (vertices, indices)
}

/// Generates a UV Sphere centered at the origin
pub fn generate_sphere(radius: f32, rings: u32, sectors: u32) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let R = 1.0 / (rings as f32);
    let S = 1.0 / (sectors as f32);

    for r in 0..=rings {
        for s in 0..=sectors {
            let theta = (r as f32) * std::f32::consts::PI * R;
            let phi = (s as f32) * 2.0 * std::f32::consts::PI * S;

            let x = theta.sin() * phi.cos();
            let y = theta.cos();
            let z = theta.sin() * phi.sin();

            let norm = Vec3::new(x, y, z);
            let pos = norm * radius;
            let uv = [(s as f32) * S, 1.0 - (r as f32) * R];

            vertices.push(Vertex::new(pos, norm, uv));
        }
    }

    for r in 0..rings {
        for s in 0..sectors {
            let idx0 = r * (sectors + 1) + s;
            let idx1 = r * (sectors + 1) + (s + 1);
            let idx2 = (r + 1) * (sectors + 1) + s;
            let idx3 = (r + 1) * (sectors + 1) + (s + 1);

            indices.push(idx0);
            indices.push(idx1);
            indices.push(idx3);

            indices.push(idx0);
            indices.push(idx3);
            indices.push(idx2);
        }
    }

    (vertices, indices)
}

/// Generates a flat quad aligned on the XZ plane
pub fn generate_plane(width: f32, depth: f32) -> (Vec<Vertex>, Vec<u32>) {
    let w = width / 2.0;
    let d = depth / 2.0;

    let norm = Vec3::new(0.0, 1.0, 0.0);

    let vertices = vec![
        Vertex::new(Vec3::new(-w, 0.0, -d), norm, [0.0, 0.0]),
        Vertex::new(Vec3::new(w, 0.0, -d), norm, [1.0, 0.0]),
        Vertex::new(Vec3::new(w, 0.0, d), norm, [1.0, 1.0]),
        Vertex::new(Vec3::new(-w, 0.0, d), norm, [0.0, 1.0]),
    ];

    let indices = vec![0, 2, 1, 0, 3, 2];

    (vertices, indices)
}

/// Generates a cylinder between two arbitrary 3D points
pub fn generate_cylinder(
    p1: Vec3,
    p2: Vec3,
    radius: f32,
    segments: u32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let d_vec = p2 - p1;
    let len = d_vec.length();
    let dir = d_vec.normalize();

    // Create orthonormal basis (dir, u, v)
    let u = if dir.x.abs() < 0.9 {
        dir.cross(Vec3::X).normalize()
    } else {
        dir.cross(Vec3::Y).normalize()
    };
    let v = dir.cross(u).normalize();

    // 1. Generate cylinder tube vertices
    for i in 0..=segments {
        let theta = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let cos_t = theta.cos();
        let sin_t = theta.sin();

        let radial_dir = u * cos_t + v * sin_t;
        let p_offset = radial_dir * radius;

        let uv_x = (i as f32) / (segments as f32);

        // Bottom vertex (on P1 plane)
        vertices.push(Vertex::new(p1 + p_offset, radial_dir, [uv_x, 0.0]));
        // Top vertex (on P2 plane)
        vertices.push(Vertex::new(p2 + p_offset, radial_dir, [uv_x, 1.0]));
    }

    // Connect tube segments
    for i in 0..segments {
        let b0 = i * 2;
        let t0 = i * 2 + 1;
        let b1 = (i + 1) * 2;
        let t1 = (i + 1) * 2 + 1;

        // Triangle 1
        indices.push(b0);
        indices.push(b1);
        indices.push(t0);

        // Triangle 2
        indices.push(t0);
        indices.push(b1);
        indices.push(t1);
    }

    // 2. Generate Bottom Cap (centered at P1, normal pointing -dir)
    let bot_center_idx = vertices.len() as u32;
    vertices.push(Vertex::new(p1, -dir, [0.5, 0.5])); // Center of bottom disk

    let bot_ring_start = vertices.len() as u32;
    for i in 0..=segments {
        let theta = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let radial_dir = u * cos_t + v * sin_t;
        let p_offset = radial_dir * radius;
        let uv_u = 0.5 + 0.5 * cos_t;
        let uv_v = 0.5 + 0.5 * sin_t;

        vertices.push(Vertex::new(p1 + p_offset, -dir, [uv_u, uv_v]));
    }

    for i in 0..segments {
        indices.push(bot_center_idx);
        indices.push(bot_ring_start + i + 1);
        indices.push(bot_ring_start + i);
    }

    // 3. Generate Top Cap (centered at P2, normal pointing +dir)
    let top_center_idx = vertices.len() as u32;
    vertices.push(Vertex::new(p2, dir, [0.5, 0.5])); // Center of top disk

    let top_ring_start = vertices.len() as u32;
    for i in 0..=segments {
        let theta = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        let radial_dir = u * cos_t + v * sin_t;
        let p_offset = radial_dir * radius;
        let uv_u = 0.5 + 0.5 * cos_t;
        let uv_v = 0.5 + 0.5 * sin_t;

        vertices.push(Vertex::new(p2 + p_offset, dir, [uv_u, uv_v]));
    }

    for i in 0..segments {
        indices.push(top_center_idx);
        indices.push(top_ring_start + i);
        indices.push(top_ring_start + i + 1);
    }

    (vertices, indices)
}
