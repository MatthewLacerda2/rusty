//! The `Skybox` source's light: the sky a scene draws, projected into L2 SH on the CPU (Unity's
//! ambient probe) at intensity 1. Derived, never saved; each source is memoised.

mod panorama;
mod procedural;

use glam::Vec3;

use crate::{Scene, Sh9};

pub(super) fn sky_radiance(scene: &Scene, color: Vec3) -> Sh9 {
    if !scene.skybox_path.is_empty() {
        if let Some(sh) = panorama::radiance(&scene.skybox_path) {
            return sh;
        }
    }
    procedural::radiance(&scene.sky, color)
}

#[cfg(test)]
mod tests;

pub use procedural::sky_color as procedural_sky_color;
