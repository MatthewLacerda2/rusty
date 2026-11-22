use glam::Vec3;

use crate::ibl::cubemap_capture::CaptureSky;
use crate::{CubemapCapture, Renderer};
use rusty_scene::Scene;

pub struct ProbeCapture {
    pub cube: CubemapCapture,
    pub open: [Vec<bool>; 6],
}

impl Renderer {
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
#[cfg(test)]
#[path = "skybox_tests.rs"]
mod probe_skybox_tests;
