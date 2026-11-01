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
mod tests;

/// A panorama for tests (#1675): a `top` colour over a `bottom` one (the horizon
/// at the image's middle row), the eastern half's red lifted by `east`, written as
/// a small PNG under the test's own temp dir. Returns its path, as `skybox_path`
/// names it.
#[cfg(test)]
pub fn panorama_fixture(name: &str, top: [u8; 3], bottom: [u8; 3], east: u8) -> String {
    let (w, h) = (64, 32);
    let image = image::RgbaImage::from_fn(w, h, |x, y| {
        let [r, g, b] = if y < h / 2 { top } else { bottom };
        // u = (atan2(z, x) + π) / 2π: +x (east) is the middle half of the columns.
        let r = if (w / 4..3 * w / 4).contains(&x) {
            r.saturating_add(east)
        } else {
            r
        };
        image::Rgba([r, g, b, 255])
    });
    let path = crate::test_temp::dir().join(name);
    image.save(&path).expect("write the panorama fixture");
    path.to_string_lossy().into_owned()
}
