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

use crate::render::{Camera, RenderView, Renderer, OFFSCREEN_FORMAT};
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
        self.capture_static_cubemap_inner(scene, position, resolution, false)
    }

    /// Like [`Renderer::capture_static_cubemap`], but the static surfaces are *also*
    /// lit by the scene's current probe field — one indirect bounce (#285). The
    /// multi-bounce probe bake calls this for bounce ≥2 after writing the previous
    /// bounce's SH into `scene.probes`, so each capture re-injects the last bounce's
    /// indirect light. Bounce 1 (and reflection bakes) use the direct-only path.
    pub fn capture_static_cubemap_lit(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
    ) -> CubemapCapture {
        self.capture_static_cubemap_inner(scene, position, resolution, true)
    }

    /// Shared capture spine: render the static scene into a 6-face cubemap from
    /// `position`. `light_static_from_probes` decides whether the static surfaces
    /// pick up the probe field (bounce ≥2) or stay direct-only (bounce 1).
    fn capture_static_cubemap_inner(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
        light_static_from_probes: bool,
    ) -> CubemapCapture {
        let resolution = resolution.max(1);
        let target = self.make_face_target(resolution);
        let output = target.create_view(&wgpu::TextureViewDescriptor::default());
        // One throwaway view (its own depth + post-FX) sized to the face, reused across
        // all six faces since they share a resolution (#355).
        let mut render_view = RenderView::targetless(
            &self.device,
            OFFSCREEN_FORMAT,
            resolution,
            resolution,
            self.quality.bloom_divisor(),
        );

        // Gather only static geometry, and (for bounce ≥2) let those static surfaces
        // sample the probe field. Both flags are restored after the six faces.
        let prev_static = self.static_capture;
        let prev_bounce = self.capture_probe_bounce;
        self.static_capture = true;
        self.capture_probe_bounce = light_static_from_probes;

        let faces = CubemapFace::ALL.map(|face| {
            let camera = face_camera(position, face);
            self.render(&mut render_view, scene, &camera, &output, false, &[]);
            crate::render::readback::read_texture_rgba8(
                &self.device,
                &self.queue,
                &target,
                resolution,
                resolution,
            )
        });

        self.static_capture = prev_static;
        self.capture_probe_bounce = prev_bounce;
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
/// exact lens that tiles six faces into a seamless cube. `pub(crate)` so the probe
/// bake (`probe_bake.rs`) reconstructs the *same* per-texel world directions the
/// capture rendered, keeping projection and capture in lockstep.
pub(crate) fn face_camera(position: Vec3, face: CubemapFace) -> Camera {
    let (yaw, pitch) = face.yaw_pitch();
    let mut camera = Camera::new(position, yaw, pitch);
    camera.fov = 90.0;
    camera
}

#[cfg(test)]
#[path = "cubemap_capture_tests.rs"]
mod cubemap_capture_tests;
