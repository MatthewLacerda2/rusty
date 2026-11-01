//! A panorama file for the `Skybox` tests (#1675), here and in the bakes'.

/// A panorama for tests (#1675): a `top` colour over a `bottom` one (the horizon
/// at the image's middle row), the eastern half's red lifted by `east`, written as
/// a small PNG under the test's own temp dir. Returns its path, as `skybox_path`
/// names it.
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
