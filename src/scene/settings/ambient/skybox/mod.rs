//! The `Skybox` source's light (#1675): the sky a scene draws, projected into L2 SH
//! on the CPU, Unity's *ambient probe*.
//!
//! The SH is the sky's **radiance**, at intensity 1, from the same source the sky
//! cube is built from: the panorama's texels ([`panorama`]) when `skybox_path` names
//! one, else the procedural sky evaluated here ([`procedural`]). It is derived state,
//! never saved: each source is projected once per change (the panorama's path or
//! file bytes, the procedural `SkySettings`) and memoised, so a frame or a bake reads
//! it for the price of a lookup. A scripted ambient colour (a day cycle tinting the
//! procedural sky) re-projects nothing: the procedural sky is affine in that colour,
//! so its SH is two fixed terms combined per call, as the sky cube does.

mod panorama;
mod procedural;

use glam::Vec3;

use crate::scene::{Scene, Sh9};

/// The radiance SH of the sky `scene` draws, at intensity 1, under ambient colour
/// `color` (what the procedural sky tints from). A panorama that cannot be read
/// falls back to the procedural sky's light.
pub(super) fn sky_radiance(scene: &Scene, color: Vec3) -> Sh9 {
    if !scene.skybox_path.is_empty() {
        if let Some(sh) = panorama::radiance(&scene.skybox_path) {
            return sh;
        }
    }
    procedural::radiance(&scene.sky, color)
}

#[cfg(test)]
mod fixture;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub use fixture::panorama_fixture;
