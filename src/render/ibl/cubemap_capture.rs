//! src/render/ibl/cubemap_capture.rs — Static-scene cubemap capture from a point (#243).
//!
//! A reusable render primitive: given a world position, render the STATIC scene into
//! a 6-face cubemap (offscreen, LDR RGBA8), reusing the forward lit pass. This is the
//! shared spine both probe bakes and reflection bakes stand on.
//!
//! Only `is_static` entities (plus, for a reflection capture, the skybox — the distant
//! environment) are drawn; dynamic actors are excluded so they never bake into a
//! probe/reflection. A light probe's capture draws no sky and reflects nothing, and
//! its bake adds the ambient where the view escapes (`probe::capture`, #1668). The static
//! filter mirrors the shadow pass's static sweep (`passes::shadows::casters::sweep`):
//! the forward solid-gather honours the renderer's [`CaptureMode::static_only`],
//! toggled on here for the six faces and restored after.
//!
//! Output is a function of (static scene, position, resolution, lens) — no wall-clock/RNG,
//! so it is exempt from (and clean under) the determinism guard. Pure render layer.

use glam::Vec3;

use super::faces::mirror_rows;
pub use super::faces::CubemapFace;
pub(crate) use super::faces::{face_camera, face_view_projection};

use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};

use crate::scene::Camera;
use crate::scene::Scene;

/// Six LDR RGBA8 faces captured from one world point. Each face is `resolution ×
/// resolution`, tightly packed (`resolution * resolution * 4` bytes), row-major top
/// to bottom, laid out as a `texture_cube` samples it (#1615): uploaded as a cube's
/// six layers, a direction reads what the capture saw along it. Indexed by
/// `CubemapFace as usize` via [`CubemapCapture::face`].
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
    /// 90° FOV per face; only `is_static` entities and the skybox are drawn. A light
    /// probe captures through [`Renderer::capture_light_probe`] instead, which draws
    /// no sky (#1668).
    ///
    /// The faces render into a target of their own, `resolution²`, whatever the
    /// renderer's own size.
    pub fn capture_static_cubemap(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
    ) -> CubemapCapture {
        self.capture_static_cubemap_inner(scene, position, resolution, CaptureSky::Drawn)
            .0
    }

    /// Like [`Renderer::capture_static_cubemap`], through a reflection probe's `lens`:
    /// its clip planes, and shadows or none (#1584).
    pub fn capture_reflection_cubemap(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
        lens: CaptureLens,
    ) -> CubemapCapture {
        let sky = CaptureSky::Reflection(lens);
        self.capture_static_cubemap_inner(scene, position, resolution, sky)
            .0
    }

    /// Shared capture spine: render the static scene into a 6-face cubemap from
    /// `position`, the sky drawn or not as `sky` says. A light-probe capture also returns, per face, which
    /// texels nothing covered (their depth is the cleared far plane): the directions
    /// that escape the scene, where `probe::capture` adds the ambient.
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
        // One throwaway view (its own depth + post-FX) sized to the face, reused across
        // all six faces since they share a resolution (#355).
        let mut render_view =
            RenderView::targetless(&self.device, OFFSCREEN_FORMAT, resolution, resolution);

        // Gather only static geometry, (for bounce ≥2) let those static surfaces
        // sample the probe field, and cast shadows as the lens says. The mode is
        // restored after the six faces.
        let prev = self.capture;
        self.capture = sky.mode();
        let lens = sky.lens();

        let mut open: [Vec<bool>; 6] = Default::default();
        let faces = CubemapFace::ALL.map(|face| {
            let mut camera = face_camera(position, face);
            (camera.near, camera.far) = (lens.near, lens.far);
            self.render_capture_face(&mut render_view, scene, &camera, &output);
            let mut pixels = crate::render::readback::read_texture_rgba8(
                &self.device,
                &self.queue,
                &target,
                resolution,
                resolution,
            );
            mirror_rows(&mut pixels, resolution);
            if self.capture.ambient_sky {
                open[face as usize] = self.read_open_texels(&render_view, resolution);
            }
            pixels
        });

        self.capture = prev;
        (CubemapCapture { resolution, faces }, open)
    }

    /// Which texels of the face just drawn into `view` nothing covered: its depth is
    /// still the far plane the scene pass cleared to. Mirrored as the colour is.
    fn read_open_texels(&self, view: &RenderView, resolution: u32) -> Vec<bool> {
        let size = (resolution, resolution);
        let mut depth = crate::render::readback::read_texture_bytes(
            &self.device,
            &self.queue,
            &view.depth_texture,
            size,
            4,
        );
        mirror_rows(&mut depth, resolution);
        depth
            .chunks_exact(4)
            .map(|d| f32::from_le_bytes([d[0], d[1], d[2], d[3]]) >= 1.0)
            .collect()
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

/// How the renderer draws while a static cubemap is captured; the default (all
/// dynamic, no probe bounce, shadows on) is every other frame's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CaptureMode {
    /// The forward pass gathers only `is_static` entities (#243), so probe and
    /// reflection bakes see the distant environment (static walls + skybox) without
    /// the dynamic actors that would otherwise bake into the captured lighting.
    pub static_only: bool,
    /// Static surfaces sample the probe field instead of flat ambient (#285): the
    /// multi-bounce probe bake sets it for bounce ≥2, adding one indirect bounce per
    /// pass. Off everywhere else, so direct-only capture and runtime shading keep
    /// their "static ⇒ no probe SH" rule.
    pub probe_bounce: bool,
    /// Lights cast shadows; off for a reflection probe captured without (#1584).
    pub shadows: bool,
    /// A light-probe capture (#1668): no sky is drawn and the background clears to
    /// black, so the escaping directions carry nothing but what the CPU adds, the
    /// ambient (Unity bakes light probes against Environment Lighting, not the skybox).
    pub ambient_sky: bool,
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

/// What a static capture sees where nothing covers the view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum CaptureSky {
    /// The skybox, through the scene camera's planes.
    Drawn,
    /// The skybox, through a reflection probe's lens (#1584).
    Reflection(CaptureLens),
    /// Nothing: a light probe's capture (#1668), the ambient added on the CPU.
    /// `bounce` lights the static surfaces from the probe field too (#285).
    Ambient { bounce: bool },
}

impl CaptureSky {
    /// The capture mode its six faces draw with.
    fn mode(self) -> CaptureMode {
        CaptureMode {
            static_only: true,
            probe_bounce: matches!(self, Self::Ambient { bounce: true }),
            shadows: self.lens().shadows,
            ambient_sky: matches!(self, Self::Ambient { .. }),
        }
    }

    fn lens(self) -> CaptureLens {
        match self {
            Self::Reflection(lens) => lens,
            Self::Drawn | Self::Ambient { .. } => CaptureLens::default(),
        }
    }
}

/// How a capture sees (#1584): the clip planes of its six cameras, and whether its
/// lights cast shadows. A reflection probe sets its own; the default is the scene
/// camera's planes, with shadows.
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
