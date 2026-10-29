//! The primitive textures the pool is dressed with: a checker floor and a rippled
//! normal map, written to the test temp dir.

use glam::Vec3;

/// A `tiles`×`tiles` black-and-white checker PNG (sRGB albedo), written once under
/// `name`; returns its path.
pub fn checker(name: &str, tiles: u32) -> String {
    let size = tiles * 8;
    let img = image::RgbaImage::from_fn(size, size, |x, y| {
        let v = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 20 };
        image::Rgba([v, v, v, 255])
    });
    save(name, img)
}

/// A tangent-space normal map of ripples along x, `waves` across the texture.
pub fn ripples(name: &str, waves: f32) -> String {
    let img = image::RgbaImage::from_fn(64, 64, |x, _| {
        let slope = (x as f32 / 64.0 * waves * std::f32::consts::TAU).sin() * 0.6;
        let n = Vec3::new(slope, 0.0, 1.0).normalize() * 0.5 + 0.5;
        let byte = |c: f32| (c * 255.0).round() as u8;
        image::Rgba([byte(n.x), byte(n.y), byte(n.z), 255])
    });
    save(name, img)
}

fn save(name: &str, img: image::RgbaImage) -> String {
    let path = crate::temp::dir().join(format!("rusty_1523_{name}.png"));
    img.save(&path).expect("write the texture");
    path.to_string_lossy().into_owned()
}
