use glam::Vec3;

use super::bounce::BakedDirect;
use super::capture::ProbeCapture;
use crate::ibl::cube_sample::{face_uv, FaceProjections};
use crate::{CubemapCapture, CubemapFace, Renderer};
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{AmbientRadiance, Scene};

/// In texels per face: an L2 SH keeps three bands, so a small cube already over-samples it.
pub const DEFAULT_BAKE_RESOLUTION: u32 = 32;

impl Renderer {
    /// A test hook: the probe tests bake one probe, direct light only.
    pub fn bake_probe(&mut self, scene: &Scene, position: Vec3, resolution: u32) -> Sh9 {
        let capture = self.capture_light_probe(scene, position, resolution, false);
        project_light_probe(&capture, scene.drawn_ambient().radiance())
    }

    /// A test hook: the dev bake's step tests.
    pub fn bake_probes(&mut self, scene: &mut Scene, resolution: u32, direct: BakedDirect) {
        self.bake_probes_multibounce(scene, resolution, direct);
    }
}

/// A test hook: the bake and HDR tests project synthetic captures.
pub fn project_cubemap(capture: &CubemapCapture) -> Sh9 {
    project(capture, |_, _, _| Vec3::ZERO)
}

pub fn project_light_probe(capture: &ProbeCapture, ambient: AmbientRadiance) -> Sh9 {
    project(&capture.cube, |face, texel, dir| {
        match capture.open[face as usize].get(texel) {
            // Clamped per direction, as the lightmap trace's escaping rays are.
            Some(true) => ambient.incoming(dir).max(Vec3::ZERO),
            _ => Vec3::ZERO,
        }
    })
}

fn project(capture: &CubemapCapture, extra: impl Fn(CubemapFace, usize, Vec3) -> Vec3) -> Sh9 {
    let res = capture.resolution.max(1);
    let mut sh = Sh9::zero();
    let proj = FaceProjections::new();
    for face in CubemapFace::ALL {
        let pixels = capture.face(face);
        for py in 0..res {
            for px in 0..res {
                let (u, v) = face_uv(px, py, res);
                let texel = (py * res + px) as usize;
                let dir = proj.direction(face, u, v);
                let rgb = pixels[texel] + extra(face, texel, dir);
                sh.add_sample(dir, rgb, texel_solid_angle(u, v, res));
            }
        }
    }
    sh
}

/// A texel spans `(2/res)²` of face space, scaled by `(1 + u² + v²)^(-3/2)` onto the sphere.
fn texel_solid_angle(u: f32, v: f32, res: u32) -> f32 {
    let texel = 2.0 / res as f32;
    let area = texel * texel;
    let d2 = 1.0 + u * u + v * v;
    area / (d2 * d2.sqrt())
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod probe_bake_tests;
