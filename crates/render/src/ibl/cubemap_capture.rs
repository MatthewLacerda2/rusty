//! Faces are read from the HDR scene target before the composite (no grade, tonemap or
//! clamp), each row-major, top to bottom.

use glam::Vec3;

use super::faces::mirror_rows;
pub use super::faces::CubemapFace;
pub(crate) use super::faces::{face_camera, face_view_projection};
use rusty_core::color::f16_to_f32;

use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};

use rusty_scene::Camera;
use rusty_scene::Scene;

pub struct CubemapCapture {
    pub resolution: u32,
    pub faces: [Vec<Vec3>; 6],
}

impl CubemapCapture {
    pub fn face(&self, face: CubemapFace) -> &[Vec3] {
        &self.faces[face as usize]
    }
}

/// The largest finite half float.
pub(crate) const HALF_MAX: f32 = 65504.0;

/// NaN to black, negatives to zero, +inf to `HALF_MAX`: one bad texel never poisons a mip or SH.
pub(crate) fn sanitize(c: f32) -> f32 {
    if c.is_nan() {
        0.0
    } else {
        c.clamp(0.0, HALF_MAX)
    }
}

pub(crate) fn decode_half_texels(bytes: &[u8]) -> Vec<Vec3> {
    let half = |b: &[u8]| sanitize(f16_to_f32(u16::from_le_bytes([b[0], b[1]])));
    (bytes.chunks_exact(8))
        .map(|t| Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])))
        .collect()
}

impl Renderer {
    pub fn capture_static_cubemap(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
    ) -> CubemapCapture {
        self.capture_static_cubemap_inner(scene, position, resolution, CaptureSky::Drawn)
            .0
    }

    pub fn capture_reflection_cubemap(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
        lens: CaptureLens,
    ) -> (CubemapCapture, [Vec<bool>; 6]) {
        let sky = CaptureSky::Reflection(lens);
        self.capture_static_cubemap_inner(scene, position, resolution, sky)
    }

    pub(crate) fn capture_static_cubemap_inner(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
        sky: CaptureSky,
    ) -> (CubemapCapture, [Vec<bool>; 6]) {
        let resolution = resolution.max(1);
        let target = self.make_face_target(resolution);
        let output = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut render_view =
            RenderView::targetless(&self.device, OFFSCREEN_FORMAT, resolution, resolution);

        let prev = self.capture;
        self.capture = sky.mode();
        let lens = sky.lens();

        let mut open: [Vec<bool>; 6] = Default::default();
        let faces = self.aside(|r| {
            CubemapFace::ALL.map(|face| {
                let mut camera = face_camera(position, face);
                (camera.near, camera.far) = (lens.near, lens.far);
                r.render_capture_face(&mut render_view, scene, &camera, &output);
                let hdr = &render_view.post_fx.scene_hdr.texture;
                let size = (resolution, resolution);
                let bytes = read_texture_bytes(&r.device, &r.queue, hdr, size, 8);
                let mut pixels = decode_half_texels(&bytes);
                mirror_rows(&mut pixels, resolution);
                if sky.reads_open() {
                    open[face as usize] = r.read_open_texels(&render_view, resolution);
                }
                pixels
            })
        });

        self.capture = prev;
        (CubemapCapture { resolution, faces }, open)
    }

    /// Texels whose depth is still the cleared far plane: directions escaping the scene.
    fn read_open_texels(&self, view: &RenderView, resolution: u32) -> Vec<bool> {
        let size = (resolution, resolution);
        let depth = read_texture_bytes(&self.device, &self.queue, &view.depth_texture, size, 4);
        let mut open: Vec<bool> = (depth.chunks_exact(4))
            .map(|d| f32::from_le_bytes([d[0], d[1], d[2], d[3]]) >= 1.0)
            .collect();
        mirror_rows(&mut open, resolution);
        open
    }

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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaptureMode {
    /// Only `is_static` entities, so dynamic actors never bake into a probe.
    pub static_only: bool,
    /// Bounce ≥2 of the probe bake: every static surface takes probe SH, not only probe readers.
    pub probe_bounce: bool,
    pub shadows: bool,
    /// A light probe's: no sky, black clear, the CPU adds the ambient, as Unity bakes them.
    pub ambient_sky: bool,
}

impl CaptureMode {
    pub(crate) fn shader_time(self, scene: &Scene) -> f32 {
        match self.static_only {
            true => 0.0,
            false => scene.shader_time,
        }
    }
}

impl Default for CaptureMode {
    fn default() -> Self {
        Self {
            static_only: false,
            probe_bounce: false,
            shadows: true,
            ambient_sky: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CaptureSky {
    Drawn,
    Reflection(CaptureLens),
    Ambient { bounce: bool },
}

impl CaptureSky {
    fn mode(self) -> CaptureMode {
        CaptureMode {
            static_only: true,
            probe_bounce: matches!(self, Self::Ambient { bounce: true }),
            shadows: self.lens().shadows,
            ambient_sky: matches!(self, Self::Ambient { .. }),
        }
    }

    fn reads_open(self) -> bool {
        !matches!(self, Self::Drawn)
    }

    fn lens(self) -> CaptureLens {
        match self {
            Self::Reflection(lens) => lens,
            Self::Drawn | Self::Ambient { .. } => CaptureLens::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptureLens {
    pub near: f32,
    pub far: f32,
    pub shadows: bool,
}

impl Default for CaptureLens {
    fn default() -> Self {
        let camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
        Self {
            near: camera.near,
            far: camera.far,
            shadows: true,
        }
    }
}

#[cfg(test)]
#[path = "cubemap_capture_tests.rs"]
mod cubemap_capture_tests;

#[cfg(test)]
#[path = "texture_cube_tests.rs"]
mod texture_cube_tests;

#[cfg(test)]
#[path = "capture_camera_tests.rs"]
mod capture_camera_tests;
