//! Each slot's encoding and packing is the slot table in `docs/api/Texture.md`: change the
//! two together.

use image::{ImageBuffer, Rgba};

use super::image_buf::Image;
use rusty_core::color::linear_to_srgb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    BaseColor,
    Emissive,
    Normal,
    Normal16,
    Roughness,
    Metallic,
    MetallicRoughness,
    Occlusion,
    Specular,
    Height,
    Data,
}

impl Slot {
    /// Every slot: a test hook for `edit`'s material-maps test.
    pub const ALL: [Slot; 11] = [
        Slot::BaseColor,
        Slot::Emissive,
        Slot::Normal,
        Slot::Normal16,
        Slot::Roughness,
        Slot::Metallic,
        Slot::MetallicRoughness,
        Slot::Occlusion,
        Slot::Specular,
        Slot::Height,
        Slot::Data,
    ];

    const NAMES: &'static str =
        "base_color (albedo), emissive, normal, normal16, roughness, metallic, metallic_roughness (orm), occlusion (ao), specular, height, data";

    /// Parse a slot name (case-insensitive, `-`/`_` ignored). An unknown name is an
    /// error listing the valid ones: a typo like `basecolour` must not
    /// silently bake linear albedo. `data` is the explicit way to get linear,
    /// unpacked output.
    pub fn parse(name: &str) -> Result<Slot, String> {
        Ok(
            match name.to_ascii_lowercase().replace(['-', '_'], "").as_str() {
                "basecolor" | "albedo" | "color" => Slot::BaseColor,
                "emissive" | "emission" => Slot::Emissive,
                "normal" => Slot::Normal,
                "normal16" => Slot::Normal16,
                "roughness" => Slot::Roughness,
                "metallic" => Slot::Metallic,
                "metallicroughness" | "mr" | "orm" => Slot::MetallicRoughness,
                "occlusion" | "ao" => Slot::Occlusion,
                "specular" => Slot::Specular,
                "height" => Slot::Height,
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

    pub fn name(self) -> &'static str {
        match self {
            Slot::BaseColor => "base_color",
            Slot::Emissive => "emissive",
            Slot::Normal => "normal",
            Slot::Normal16 => "normal16",
            Slot::Roughness => "roughness",
            Slot::Metallic => "metallic",
            Slot::MetallicRoughness => "metallic_roughness",
            Slot::Occlusion => "occlusion",
            Slot::Specular => "specular",
            Slot::Height => "height",
            Slot::Data => "data",
        }
    }

    pub fn material_maps(self) -> &'static [&'static str] {
        match self {
            Slot::BaseColor => &["base_color_map"],
            Slot::Emissive => &["emissive_map"],
            Slot::Normal | Slot::Normal16 => &["normal_map"],
            Slot::Roughness => &["roughness_map"],
            Slot::Metallic => &["metallic_map"],
            Slot::MetallicRoughness => &["metallic_map", "roughness_map"],
            Slot::Occlusion => &["occlusion_map"],
            Slot::Specular => &["specular_map"],
            Slot::Height => &["height_map"],
            Slot::Data => &[],
        }
    }

    pub fn kind(self) -> Slot {
        match self {
            Slot::Normal16 => Slot::Normal,
            other => other,
        }
    }

    fn is_srgb(self) -> bool {
        matches!(self, Slot::BaseColor | Slot::Emissive)
    }
}

fn encode_pixel(p: [f32; 4], slot: Slot) -> [u8; 4] {
    let packed = match slot {
        Slot::MetallicRoughness => [0.0, p[1], p[0], p[3]],
        Slot::Specular => [p[0]; 4],
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
    out[3] = to_u8(packed[3].clamp(0.0, 1.0));
    out
}

/// `[0, 1]` to a byte, rounded to nearest.
#[inline]
fn to_u8(v: f32) -> u8 {
    (v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

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

pub fn encode_data(img: &Image) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let raw = img
        .pixels()
        .iter()
        .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16))
        .collect();
    ImageBuffer::from_raw(img.width(), img.height(), raw).expect("one RGBA texel per pixel")
}

pub fn height_of(img: &Image) -> Image {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        *p = [p[0], p[0], p[0], 1.0];
    }
    out
}

pub fn bake_to_png(img: &Image, slot: Slot, path: &str) -> Result<String, String> {
    match slot {
        Slot::Data | Slot::Normal16 => encode_data(img).save(path),
        Slot::Height => encode_data(&height_of(img)).save(path),
        _ => encode(img, slot).save(path),
    }
    .map_err(|e| format!("cannot write {path}: {e}"))?;
    // Noted so a renderer that already drew the old file re-reads it.
    rusty_core::fs::written::note_written(path);
    Ok(path.to_string())
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod tests;
