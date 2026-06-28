//! src/procgen/bake.rs — encode an evaluated buffer to a PNG for a material slot.
//!
//! The runner produces a linear `[f32; 4]` buffer; the bake turns it into an 8-bit
//! RGBA `.png` the renderer's `load_texture` already consumes. The crucial step is
//! **per-slot encoding**, so a baked map drops straight into a glTF material slot
//! with the right color space and channel layout:
//!
//! - **Albedo / base-color and emissive** are **sRGB-encoded** (the renderer uploads
//!   them as `Rgba8UnormSrgb`, expecting sRGB bytes).
//! - **Normal, roughness, metallic, height/data** are **linear** (no gamma) — they
//!   carry data, not color.
//! - **Metallic-roughness** packs **metallic → B**, **roughness → G** (the glTF ORM
//!   convention), so one bake fills a packed MR map; R/A are passed through.
//! - **Normal** maps are tangent-space (produced by [`super::ops::vector::bump_to_normal`]).
//!
//! The agent picks the slot when baking, so it declares *what kind of map* it is
//! producing and the encoding follows.

use image::{ImageBuffer, Rgba};

use super::image_buf::Image;

/// The material slot a bake targets — selects color space and channel packing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Base color / albedo — sRGB-encoded.
    BaseColor,
    /// Emissive — sRGB-encoded.
    Emissive,
    /// Tangent-space normal map — linear, channels passed through.
    Normal,
    /// Standalone linear roughness map.
    Roughness,
    /// Standalone linear metallic map.
    Metallic,
    /// Packed metallic-roughness: metallic→B, roughness→G (glTF ORM). The buffer's
    /// red is taken as metallic and green as roughness; we route them to B and G.
    MetallicRoughness,
    /// Generic linear data/height map — no encoding, no repack.
    Data,
}

impl Slot {
    /// Parse a slot name (case-insensitive). Unknown names map to `Data` (linear,
    /// unpacked) so a typo degrades safely rather than silently sRGB-encoding data.
    pub fn parse(name: &str) -> Slot {
        match name.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
            "basecolor" | "albedo" | "color" => Slot::BaseColor,
            "emissive" | "emission" => Slot::Emissive,
            "normal" => Slot::Normal,
            "roughness" => Slot::Roughness,
            "metallic" => Slot::Metallic,
            "metallicroughness" | "mr" | "orm" => Slot::MetallicRoughness,
            _ => Slot::Data,
        }
    }

    /// Whether this slot's color channels are sRGB-encoded on bake.
    fn is_srgb(self) -> bool {
        matches!(self, Slot::BaseColor | Slot::Emissive)
    }
}

/// Linear → sRGB transfer (IEC 61966-2-1), for a single channel in `[0, 1]`.
fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert one linear pixel to the 8-bit RGBA bytes for `slot`, applying the slot's
/// encoding and channel packing.
fn encode_pixel(p: [f32; 4], slot: Slot) -> [u8; 4] {
    // Channel packing first: a packed MR map routes metallic(R)→B, roughness(G)→G.
    let packed = match slot {
        Slot::MetallicRoughness => [0.0, p[1], p[0], p[3]],
        _ => p,
    };
    let mut out = [0u8; 4];
    for c in 0..3 {
        let v = if slot.is_srgb() {
            linear_to_srgb(packed[c])
        } else {
            packed[c].clamp(0.0, 1.0)
        };
        out[c] = to_u8(v);
    }
    // Alpha is always linear.
    out[3] = to_u8(packed[3].clamp(0.0, 1.0));
    out
}

/// `[0, 1]` float → byte, rounding to nearest (so 1.0 → 255, 0.5 → 128).
#[inline]
fn to_u8(v: f32) -> u8 {
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

/// Encode `img` for `slot` into an 8-bit RGBA [`ImageBuffer`] — the in-memory PNG
/// pixels, before writing. Exposed so tests can assert bytes without disk I/O.
pub fn encode(img: &Image, slot: Slot) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let res = img.resolution();
    let mut buf = ImageBuffer::new(res, res);
    for (i, p) in img.pixels().iter().enumerate() {
        let x = (i as u32) % res;
        let y = (i as u32) / res;
        buf.put_pixel(x, y, Rgba(encode_pixel(*p, slot)));
    }
    buf
}

/// Bake `img` to a `.png` at `path`, encoded for `slot`. Returns the path on success.
pub fn bake_to_png(img: &Image, slot: Slot, path: &str) -> Result<String, String> {
    let buf = encode(img, slot);
    buf.save(path).map_err(|e| e.to_string())?;
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_parse_is_case_and_separator_insensitive() {
        assert_eq!(Slot::parse("BaseColor"), Slot::BaseColor);
        assert_eq!(Slot::parse("albedo"), Slot::BaseColor);
        assert_eq!(Slot::parse("metallic-roughness"), Slot::MetallicRoughness);
        assert_eq!(Slot::parse("orm"), Slot::MetallicRoughness);
        assert_eq!(Slot::parse("nonsense"), Slot::Data);
    }

    #[test]
    fn srgb_encoding_applied_only_to_color_slots() {
        // Mid linear gray (0.5) encodes brighter under sRGB (~0.735 → 188).
        let img = Image::filled(2, [0.5, 0.5, 0.5, 1.0]);
        let color = encode(&img, Slot::BaseColor).get_pixel(0, 0).0;
        let data = encode(&img, Slot::Data).get_pixel(0, 0).0;
        assert_eq!(data[0], 128, "linear data: 0.5 -> 128");
        assert!(
            color[0] > 180 && color[0] < 195,
            "sRGB 0.5 ~ 188, got {}",
            color[0]
        );
    }

    #[test]
    fn metallic_roughness_packs_metallic_to_b_roughness_to_g() {
        // Buffer red = metallic = 1.0, green = roughness = 0.25.
        let img = Image::filled(2, [1.0, 0.25, 0.0, 1.0]);
        let px = encode(&img, Slot::MetallicRoughness).get_pixel(0, 0).0;
        assert_eq!(px[2], 255, "metallic -> B");
        assert_eq!(px[1], to_u8(0.25), "roughness -> G");
        assert_eq!(px[0], 0, "R cleared in the pack");
    }

    #[test]
    fn linear_to_srgb_endpoints() {
        assert!((linear_to_srgb(0.0)).abs() < 1e-6);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);
    }
}
