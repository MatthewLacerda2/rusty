//! src/render/cubemap_capture.rs — Static-scene cubemap capture from a point (#243).
//!
//! A reusable render primitive: given a world position, render the STATIC scene into
//! a 6-face cubemap (offscreen, LDR RGBA8), reusing the forward lit pass. This is the
//! shared spine both probe bakes and reflection bakes stand on.
//!
//! Only `is_static` entities (plus the skybox — the distant environment) are drawn;
//! dynamic actors are excluded so they never bake into a probe/reflection. The static
//! filter mirrors the shadow pass's `want_static` pattern (`shadows.rs`): the forward
//! solid-gather honours the renderer's `static_capture` flag, toggled on here for the
//! six faces and restored after.
//!
//! Output is a function of (static scene, position, resolution) — no wall-clock/RNG,
//! so it is exempt from (and clean under) the determinism guard. Pure render layer.

use glam::Vec3;

use super::{Camera, Renderer, OFFSCREEN_FORMAT};
use crate::scene::Scene;

/// The six faces of a cubemap, in the conventional +X,-X,+Y,-Y,+Z,-Z order. The
/// index into [`CubemapCapture::faces`] is `face as usize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubemapFace {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl CubemapFace {
    /// All six faces in storage order.
    pub const ALL: [CubemapFace; 6] = [
        CubemapFace::PosX,
        CubemapFace::NegX,
        CubemapFace::PosY,
        CubemapFace::NegY,
        CubemapFace::PosZ,
        CubemapFace::NegZ,
    ];

    /// The (yaw, pitch) in degrees that aims [`Camera`] down this face's axis, so the
    /// `PosX` face looks toward +X, etc. The view basis [`Camera::build_view_projection`]
    /// derives from `look_at_rh` flips the apparent axis relative to [`Camera::forward`]'s
    /// `forward = (cosY·cosP, sinP, sinY·cosP)`, so each face aims down the *opposite*
    /// yaw/pitch (verified against the rendered face colours, #243).
    fn yaw_pitch(self) -> (f32, f32) {
        match self {
            CubemapFace::PosX => (180.0, 0.0),
            CubemapFace::NegX => (0.0, 0.0),
            CubemapFace::PosY => (0.0, -90.0),
            CubemapFace::NegY => (0.0, 90.0),
            CubemapFace::PosZ => (-90.0, 0.0),
            CubemapFace::NegZ => (90.0, 0.0),
        }
    }
}

/// Six LDR RGBA8 faces captured from one world point. Each face is `resolution ×
/// resolution`, tightly packed (`resolution * resolution * 4` bytes), row-major top
/// to bottom. Indexed by `CubemapFace as usize` via [`CubemapCapture::face`].
pub struct CubemapCapture {
    /// Edge length in pixels of every face.
    pub resolution: u32,
    /// The six faces in `CubemapFace::ALL` order.
    pub faces: [Vec<u8>; 6],
}

impl CubemapCapture {
    /// The packed RGBA8 bytes of one face.
    pub fn face(&self, face: CubemapFace) -> &[u8] {
        &self.faces[face as usize]
    }
}

impl Renderer {
    /// Render the STATIC scene into a 6-face cubemap centred on `position`, each face
    /// `resolution × resolution` LDR RGBA8 (#243). Reuses the forward lit pass with a
    /// 90° FOV per face; only `is_static` entities and the skybox are drawn.
    ///
    /// `self` must be a renderer sized at least `resolution × resolution` (a headless
    /// renderer built at that size — see [`Renderer::new_headless`]); faces are read
    /// from the top-left `resolution²` region of its offscreen target.
    pub fn capture_static_cubemap(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
    ) -> CubemapCapture {
        let resolution = resolution.max(1);
        let target = self.make_face_target(resolution);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        // Gather only static geometry for the duration of the capture, then restore.
        let prev_static = self.static_capture;
        self.static_capture = true;

        let faces = CubemapFace::ALL.map(|face| {
            let camera = face_camera(position, face);
            self.render(scene, &camera, &view, false, &[]);
            read_face(self, &target, resolution)
        });

        self.static_capture = prev_static;
        CubemapCapture { resolution, faces }
    }

    /// Build the per-face offscreen colour target (LDR RGBA8, copyable to CPU).
    fn make_face_target(&self, resolution: u32) -> wgpu::Texture {
        self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cubemap Face Target"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }
}

/// A [`Camera`] at `position` aimed down `face`'s axis with a 90° vertical FOV — the
/// exact lens that tiles six faces into a seamless cube.
fn face_camera(position: Vec3, face: CubemapFace) -> Camera {
    let (yaw, pitch) = face.yaw_pitch();
    let mut camera = Camera::new(position, yaw, pitch);
    camera.fov = 90.0;
    camera
}

/// Copy the rendered face texture back to the CPU as tightly-packed RGBA8 bytes.
/// `copy_texture_to_buffer` requires each row padded to `COPY_BYTES_PER_ROW_ALIGNMENT`
/// (256), so we copy into a padded buffer and strip the padding back out — the same
/// readback the screenshot path uses.
fn read_face(renderer: &Renderer, target: &wgpu::Texture, resolution: u32) -> Vec<u8> {
    let unpadded_bytes_per_row = resolution * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;

    let buffer = copy_face_to_buffer(renderer, target, resolution, padded_bytes_per_row);

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    renderer.device.poll(wgpu::Maintain::Wait);
    let _ = rx.recv();

    let mapped = slice.get_mapped_range();
    let mut pixels = Vec::with_capacity((unpadded_bytes_per_row * resolution) as usize);
    for row in mapped.chunks(padded_bytes_per_row as usize) {
        pixels.extend_from_slice(&row[..unpadded_bytes_per_row as usize]);
    }
    drop(mapped);
    buffer.unmap();
    pixels
}

/// Allocate a padded readback buffer and copy the rendered face texture into it.
fn copy_face_to_buffer(
    renderer: &Renderer,
    target: &wgpu::Texture,
    resolution: u32,
    padded_bytes_per_row: u32,
) -> wgpu::Buffer {
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Cubemap Face Readback Buffer"),
        size: (padded_bytes_per_row * resolution) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = renderer
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cubemap Face Copy Encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::ImageCopyTexture {
            texture: target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::ImageCopyBuffer {
            buffer: &buffer,
            layout: wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(resolution),
            },
        },
        wgpu::Extent3d {
            width: resolution,
            height: resolution,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit(std::iter::once(encoder.finish()));
    buffer
}

#[cfg(test)]
#[path = "cubemap_capture_tests.rs"]
mod cubemap_capture_tests;
