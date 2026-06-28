//! src/render/probe_bake.rs — Light-probe capture + SH projection (#241).
//!
//! The rendered-capture counterpart to the analytic stand-in (`scene::probe_fill`).
//! For each probe we capture the STATIC scene to a 6-face cubemap from the probe's
//! position (`Renderer::capture_static_cubemap`, #243), then project that captured
//! radiance into an L2 irradiance SH (`scene::sh`, #240): every texel is decoded to
//! linear RGB, aimed down its true world direction, and accumulated weighted by its
//! cubemap solid angle (cosine-lobe convolution then happens on read-back in
//! `Sh9::eval`). The 27 floats per probe land in `Scene::probes`, ready for the
//! `<scene>.lighting.json` sidecar the runtime already loads.
//!
//! A single capture/projection here is ONE bounce (the static scene lit by direct
//! light only). The true multi-bounce solve that iterates these passes and folds the
//! probe field back into each capture lives in `probe_bounce.rs` (#285); `bake_probes`
//! drives it.
//!
//! Pure render layer: a function of (static scene, probe positions, resolution),
//! no wall-clock / RNG — exempt from (and clean under) the determinism guard, like
//! the capture primitive it stands on.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

use crate::render::ibl::cubemap_capture::face_camera;
use crate::render::{CubemapCapture, CubemapFace, Renderer};
use crate::scene::lighting::sh::Sh9;
use crate::scene::Scene;

/// Default per-face capture resolution for a probe bake. Small is plenty: an L2 SH
/// keeps only 3 bands, so a low-res cubemap already over-samples the signal, and the
/// solid-angle weighting makes the projection resolution-insensitive.
pub const DEFAULT_BAKE_RESOLUTION: u32 = 32;

impl Renderer {
    /// Bake one probe: capture the static scene from `position` and project the
    /// captured radiance into an L2 irradiance SH. `resolution` is the per-face edge
    /// length (clamped to the renderer's offscreen size by the capture primitive).
    pub fn bake_probe(&mut self, scene: &Scene, position: Vec3, resolution: u32) -> Sh9 {
        let capture = self.capture_static_cubemap(scene, position, resolution);
        project_cubemap(&capture)
    }

    /// Bake every probe in `scene.probes` with the multi-bounce GI solve (#285),
    /// writing each probe's converged SH in place. The probe positions and grid layout
    /// are untouched — only the SH coefficients change, so a later
    /// `save_lighting_sidecar` persists the bounce next to the scene. Thin wrapper over
    /// [`Renderer::bake_probes_multibounce`] (the iteration lives in `probe_bounce.rs`);
    /// the discarded [`crate::render::ibl::probe_bounce::BounceReport`] is available
    /// from that method when the caller wants the bounce count.
    pub fn bake_probes(&mut self, scene: &mut Scene, resolution: u32) {
        self.bake_probes_multibounce(scene, resolution);
    }
}

/// Project a captured cubemap's radiance into an L2 irradiance SH by summing every
/// texel of every face, each aimed down its world direction and weighted by its
/// cubemap solid angle. The returned `Sh9` reads back as diffuse irradiance through
/// the cosine-lobe convolution baked into `Sh9::eval`.
pub fn project_cubemap(capture: &CubemapCapture) -> Sh9 {
    let res = capture.resolution.max(1);
    let mut sh = Sh9::zero();
    for face in CubemapFace::ALL {
        accumulate_face(&mut sh, capture.face(face), face, res);
    }
    sh
}

/// Accumulate one face's texels into `sh`. Each texel contributes its decoded linear
/// radiance aimed down `texel_dir`, weighted by the texel's solid angle.
fn accumulate_face(sh: &mut Sh9, pixels: &[u8], face: CubemapFace, res: u32) {
    let basis = FaceBasis::new(face);
    for py in 0..res {
        for px in 0..res {
            let (u, v) = face_uv(px, py, res);
            let dir = basis.direction(u, v);
            let weight = texel_solid_angle(u, v, res);
            let idx = ((py * res + px) * 4) as usize;
            let rgb = decode_linear(&pixels[idx..idx + 3]);
            sh.add_sample(dir, rgb, weight);
        }
    }
}

/// Reconstructs the world direction each face texel actually saw, by inverting the
/// exact view-projection `capture_static_cubemap` rendered the face with. Using the
/// real matrix (not a hand-rolled basis) keeps projection in lockstep with the
/// capture's empirically-tuned per-face orientation (#243) — a texel's direction is
/// always the ray the GPU sampled for that pixel, flips and all.
struct FaceBasis {
    inv_view_proj: Mat4,
    eye: Vec3,
}

impl FaceBasis {
    fn new(face: CubemapFace) -> Self {
        let camera = face_camera(Vec3::ZERO, face);
        // Square faces → aspect 1. The capture renders at this exact lens.
        let view_proj = camera.build_view_projection(1.0);
        Self {
            inv_view_proj: view_proj.inverse(),
            eye: camera.position,
        }
    }

    /// World direction for a face texel at face coords `(u, v)` in [-1, 1], which are
    /// the clip-space NDC of that pixel (x right, y up). Unproject a far-plane point
    /// and take the ray from the eye through it.
    fn direction(&self, u: f32, v: f32) -> Vec3 {
        let clip = Vec4::new(u, v, 1.0, 1.0);
        let world = self.inv_view_proj * clip;
        let point = world.xyz() / world.w;
        (point - self.eye).normalize_or_zero()
    }
}

/// Face coords in [-1, 1] for pixel `(px, py)` of a `res`-wide face. `u` runs right,
/// `v` runs up (image rows are top-to-bottom, so the top row maps to `v ≈ +1`).
fn face_uv(px: u32, py: u32, res: u32) -> (f32, f32) {
    let u = ((px as f32 + 0.5) / res as f32) * 2.0 - 1.0;
    let v = 1.0 - ((py as f32 + 0.5) / res as f32) * 2.0;
    (u, v)
}

/// Differential solid angle of a cube texel at face coords `(u, v)`. A texel spans
/// `(2/res)²` in face space; projected onto the unit sphere it scales by
/// `(1 + u² + v²)^(-3/2)`. Summed over all six faces this integrates to ~4π.
fn texel_solid_angle(u: f32, v: f32, res: u32) -> f32 {
    let texel = 2.0 / res as f32;
    let area = texel * texel;
    let d2 = 1.0 + u * u + v * v;
    area / (d2 * d2.sqrt())
}

/// Decode an LDR sRGB-encoded RGB texel (the tonemapped capture, `Rgba8Unorm`) to
/// linear radiance, so a red wall projects as red light rather than its gamma-coded
/// byte value.
fn decode_linear(bytes: &[u8]) -> Vec3 {
    Vec3::new(
        srgb_to_linear(bytes[0]),
        srgb_to_linear(bytes[1]),
        srgb_to_linear(bytes[2]),
    )
}

/// Inverse-sRGB transfer for one 8-bit channel.
fn srgb_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
#[path = "probe_bake_tests.rs"]
mod probe_bake_tests;
