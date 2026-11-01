//! A light probe's capture (#1668): the static scene with no sky, and the
//! directions that escape it.
//!
//! Unity bakes light probes against *Environment Lighting* (the ambient source,
//! Color or Gradient), never against the skybox the camera draws: the skybox
//! reaches surfaces only through reflections. So a light-probe capture draws no
//! sky (`CaptureSky::Ambient`: the background clears to black) and keeps, per
//! face, which texels nothing covered. The projection (`bake::project_light_probe`)
//! gives those texels the ambient's radiance along their direction,
//! `AmbientRadiance::incoming`, on the CPU: unclamped and before any tonemap or
//! 8-bit encode, so an open probe's SH is the one `scene::lighting::probe_fill`
//! projects, and a probe-lit object agrees with the lightmapped and unbaked
//! surfaces beside it (#1647's decision 3). Reflection probes keep the sky.

use glam::Vec3;

use crate::render::ibl::cubemap_capture::CaptureSky;
use crate::render::{CubemapCapture, Renderer};
use crate::scene::Scene;

/// One light probe's capture: the six faces, and which of their texels escape.
pub struct ProbeCapture {
    pub cube: CubemapCapture,
    /// Per face (`CubemapFace::ALL` order), one flag per texel, row-major like the
    /// face: `true` where nothing covered it, a direction that escapes the scene.
    pub open: [Vec<bool>; 6],
}

impl Renderer {
    /// Capture the static scene for a light probe at `position`, without the sky.
    /// `bounce` lights the static surfaces from the scene's probe field as well, one
    /// indirect bounce (#285): the multi-bounce solve's passes from the second on.
    pub fn capture_light_probe(
        &mut self,
        scene: &Scene,
        position: Vec3,
        resolution: u32,
        bounce: bool,
    ) -> ProbeCapture {
        let sky = CaptureSky::Ambient { bounce };
        let (cube, open) = self.capture_static_cubemap_inner(scene, position, resolution, sky);
        ProbeCapture { cube, open }
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod probe_capture_tests;
