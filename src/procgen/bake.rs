//! src/procgen/bake.rs — encode an evaluated buffer to a PNG for a material slot.
//!
//! The runner produces a linear `[f32; 4]` buffer; the bake turns it into an RGBA
//! `.png` the renderer's `load_texture` already consumes: 8 bits per channel for
//! every slot but `data`, which bakes 16 (#1491) so a height field used as
//! displacement or a terrain heightmap doesn't terrace into 256 steps. The crucial step is
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
use crate::core::color::linear_to_srgb;

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
    /// The canonical slot names, as an unknown-slot error lists them.
    const NAMES: &'static str =
        "base_color (albedo), emissive, normal, roughness, metallic, metallic_roughness (orm), data";

    /// Parse a slot name (case-insensitive, `-`/`_` ignored). An unknown name is an
    /// error listing the valid ones (#395): a typo like `basecolour` must not
    /// silently bake linear albedo. `data` is the explicit way to get linear,
    /// unpacked output.
    pub fn parse(name: &str) -> Result<Slot, String> {
        Ok(
            match name.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
                "basecolor" | "albedo" | "color" => Slot::BaseColor,
                "emissive" | "emission" => Slot::Emissive,
                "normal" => Slot::Normal,
                "roughness" => Slot::Roughness,
                "metallic" => Slot::Metallic,
                "metallicroughness" | "mr" | "orm" => Slot::MetallicRoughness,
                "data" => Slot::Data,
                _ => {
                    return Err(format!(
                        "unknown slot {name:?}; expected one of {}",
                        Self::NAMES
                    ))
                }
            },
        )
    }

    /// The canonical name of the slot — the key `Texture.BakeSet` returns its path
    /// under and the suffix of its file name (`<prefix>_<name>.png`).
    pub fn name(self) -> &'static str {
        match self {
            Slot::BaseColor => "base_color",
            Slot::Emissive => "emissive",
            Slot::Normal => "normal",
            Slot::Roughness => "roughness",
            Slot::Metallic => "metallic",
            Slot::MetallicRoughness => "metallic_roughness",
            Slot::Data => "data",
        }
    }

    /// The `Material.DefineAsset` map keys this slot's PNG fills (#409): a packed
    /// metallic-roughness map fills both `metallic_map` and `roughness_map` (the
    /// shader reads B and G from it); `data` feeds no material slot.
    pub fn material_maps(self) -> &'static [&'static str] {
        match self {
            Slot::BaseColor => &["base_color_map"],
            Slot::Emissive => &["emissive_map"],
            Slot::Normal => &["normal_map"],
            Slot::Roughness => &["roughness_map"],
            Slot::Metallic => &["metallic_map"],
            Slot::MetallicRoughness => &["metallic_map", "roughness_map"],
            Slot::Data => &[],
        }
    }

    /// Whether this slot's color channels are sRGB-encoded on bake.
    fn is_srgb(self) -> bool {
        matches!(self, Slot::BaseColor | Slot::Emissive)
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
/// pixels, before writing, for every slot but [`Slot::Data`], which bakes through
/// [`encode_data`]. Exposed so tests can assert bytes without disk I/O.
pub fn encode(img: &Image, slot: Slot) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let w = img.width();
    let mut buf = ImageBuffer::new(w, img.height());
    for (i, p) in img.pixels().iter().enumerate() {
        let x = (i as u32) % w;
        let y = (i as u32) / w;
        buf.put_pixel(x, y, Rgba(encode_pixel(*p, slot)));
    }
    buf
}

/// Encode `img` as a [`Slot::Data`] map: 16-bit linear RGBA, each channel clamped
/// to `[0, 1]` and rounded to the nearest of 65536 steps.
pub fn encode_data(img: &Image) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let raw = img
        .pixels()
        .iter()
        .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16))
        .collect();
    ImageBuffer::from_raw(img.width(), img.height(), raw).expect("one RGBA texel per pixel")
}

/// Bake `img` to a `.png` at `path`, encoded for `slot`, and note the write in
/// [`crate::core::fs::written`] so a renderer that already drew the old file re-reads
/// it (#689). Returns the path.
pub fn bake_to_png(img: &Image, slot: Slot, path: &str) -> Result<String, String> {
    match slot {
        Slot::Data => encode_data(img).save(path),
        _ => encode(img, slot).save(path),
    }
    .map_err(|e| e.to_string())?;
    crate::core::fs::written::note_written(path);
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_parse_is_case_and_separator_insensitive() {
        assert_eq!(Slot::parse("BaseColor"), Ok(Slot::BaseColor));
        assert_eq!(Slot::parse("albedo"), Ok(Slot::BaseColor));
        assert_eq!(
            Slot::parse("metallic-roughness"),
            Ok(Slot::MetallicRoughness)
        );
        assert_eq!(Slot::parse("orm"), Ok(Slot::MetallicRoughness));
        assert_eq!(Slot::parse("data"), Ok(Slot::Data));
    }

    #[test]
    fn an_unknown_slot_is_refused_by_name() {
        let err = Slot::parse("basecolour").unwrap_err();
        assert!(err.contains("\"basecolour\""), "{err}");
        assert!(err.contains("base_color") && err.contains("data"), "{err}");
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
    fn a_data_ramp_round_trips_at_16_bit_precision() {
        // A 0..1 ramp across 256² pixels: 65536 distinct values, past what 8 bits hold.
        let res = 256;
        let n = (res * res) as f32;
        let mut img = Image::new(res);
        for (i, p) in img.pixels_mut().iter_mut().enumerate() {
            *p = [i as f32 / (n - 1.0); 4];
        }
        let path = crate::test_temp::dir().join("rusty_bake_data_ramp_1491.png");
        bake_to_png(&img, Slot::Data, path.to_str().unwrap()).unwrap();
        let back = image::open(&path).unwrap();
        assert_eq!(back.color(), image::ColorType::Rgba16, "data bakes 16-bit");
        let back = back.to_rgba16();
        for (p, q) in img.pixels().iter().zip(back.pixels()) {
            let read = f32::from(q.0[0]) / 65535.0;
            assert!(
                (read - p[0]).abs() <= 0.5 / 65535.0 + 1e-7,
                "{} vs {read}",
                p[0]
            );
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn colour_slots_stay_8_bit() {
        let path = crate::test_temp::dir().join("rusty_bake_colour_8bit_1491.png");
        bake_to_png(&Image::new(2), Slot::BaseColor, path.to_str().unwrap()).unwrap();
        assert_eq!(image::open(&path).unwrap().color(), image::ColorType::Rgba8);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn linear_to_srgb_endpoints() {
        assert!((linear_to_srgb(0.0)).abs() < 1e-6);
        assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);
    }
}
