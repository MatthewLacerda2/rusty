//! crates/procgen/src/bake.rs — encode an evaluated buffer to a PNG for a material slot.
//!
//! The runner produces a linear `[f32; 4]` buffer; the bake turns it into an RGBA
//! `.png` the renderer's `load_texture` already consumes: 8 bits per channel for
//! every slot but `data`, which bakes 16 (#1491) so a height field used as
//! displacement or a terrain heightmap doesn't terrace into 256 steps, and
//! `normal16`, a normal map at 16 (#2002) for relief subtler than one 8-bit step. The crucial step is
//! **per-slot encoding**, so a baked map drops straight into a glTF material slot
//! with the right color space and channel layout:
//!
//! - **Albedo / base-color and emissive** are **sRGB-encoded** (the renderer uploads
//!   them as `Rgba8UnormSrgb`, expecting sRGB bytes).
//! - **Normal, roughness, metallic, height/data** are **linear** (no gamma) — they
//!   carry data, not color.
//! - **Metallic-roughness** packs **metallic → B**, **roughness → G** (the glTF ORM
//!   convention), so one bake fills a packed MR map; R/A are passed through.
//! - **Occlusion** is linear and passed through (glTF reads its R); a **specular**
//!   level is linear and its R is copied to every channel, since glTF reads it
//!   from A (#2151).
//! - **Height** is linear at 16 bits, its R copied to RGB and A opaque (#2191): the
//!   material's parallax marches its red, and 8 bits would terrace the march.
//! - **Normal** maps are tangent-space (produced by [`super::ops::vector::bump_to_normal`]).
//!
//! The agent picks the slot when baking, so it declares *what kind of map* it is
//! producing and the encoding follows.

use image::{ImageBuffer, Rgba};

use super::image_buf::Image;
use rusty_core::color::linear_to_srgb;

/// The material slot a bake targets — selects color space and channel packing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// Base color / albedo — sRGB-encoded.
    BaseColor,
    /// Emissive — sRGB-encoded.
    Emissive,
    /// Tangent-space normal map — linear, channels passed through.
    Normal,
    /// [`Slot::Normal`] at 16 bits per channel (#2002): a slope under about 1/255
    /// that 8 bits round flat survives. It fills `normal_map` as `normal` does.
    Normal16,
    /// Standalone linear roughness map.
    Roughness,
    /// Standalone linear metallic map.
    Metallic,
    /// Packed metallic-roughness: metallic→B, roughness→G (glTF ORM). The buffer's
    /// red is taken as metallic and green as roughness; we route them to B and G.
    MetallicRoughness,
    /// Linear ambient occlusion (#2151), glTF's `occlusionTexture`: channels passed
    /// through, the material reading R.
    Occlusion,
    /// Linear specular level (#2151), `KHR_materials_specular`'s `specularTexture`:
    /// the buffer's red written to every channel, the material reading A.
    Specular,
    /// Linear height (#2191), the material's `height_map`: 16 bits, the buffer's red
    /// in RGB (white the surface, black the deepest point) and A opaque.
    Height,
    /// Generic linear data/height map — no encoding, no repack.
    Data,
}

impl Slot {
    /// Every slot, in declaration order: what a caller walks to check each one
    /// (`edit::authoring::material::maps`' test that every map key it fills is a
    /// field). `bake_tests` holds it whole by an exhaustive `match`.
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

    /// The canonical slot names, as an unknown-slot error lists them.
    const NAMES: &'static str =
        "base_color (albedo), emissive, normal, normal16, roughness, metallic, metallic_roughness (orm), occlusion (ao), specular, height, data";

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

    /// The canonical name of the slot — the key `Texture.BakeSet` returns its path
    /// under and the suffix of its file name (`<prefix>_<name>.png`).
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

    /// The `Material.DefineAsset` map keys this slot's PNG fills (#409): a packed
    /// metallic-roughness map fills both `metallic_map` and `roughness_map` (the
    /// shader reads B and G from it); `data` feeds no material slot.
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

    /// The slot a set may name once: a slot at another bit depth is that slot, so
    /// `normal` and `normal16` in one set are the normal map twice.
    pub fn kind(self) -> Slot {
        match self {
            Slot::Normal16 => Slot::Normal,
            other => other,
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
/// pixels, before writing, for every slot but [`Slot::Data`], [`Slot::Normal16`] and
/// [`Slot::Height`], which bake through [`encode_data`] ([`height_of`] first). Exposed so tests can assert bytes without disk I/O.
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

/// Encode `img` as a [`Slot::Data`] or [`Slot::Normal16`] map: 16-bit linear RGBA,
/// each channel clamped to `[0, 1]` and rounded to the nearest of 65536 steps.
pub fn encode_data(img: &Image) -> ImageBuffer<Rgba<u16>, Vec<u16>> {
    let raw = img
        .pixels()
        .iter()
        .flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16))
        .collect();
    ImageBuffer::from_raw(img.width(), img.height(), raw).expect("one RGBA texel per pixel")
}

/// `img` as a [`Slot::Height`] map holds it: its red in RGB, alpha 1.
pub fn height_of(img: &Image) -> Image {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        *p = [p[0], p[0], p[0], 1.0];
    }
    out
}

/// Bake `img` to a `.png` at `path`, encoded for `slot`, and note the write in
/// [`rusty_core::fs::written`] so a renderer that already drew the old file re-reads
/// it (#689). Returns the path.
pub fn bake_to_png(img: &Image, slot: Slot, path: &str) -> Result<String, String> {
    match slot {
        Slot::Data | Slot::Normal16 => encode_data(img).save(path),
        Slot::Height => encode_data(&height_of(img)).save(path),
        _ => encode(img, slot).save(path),
    }
    .map_err(|e| format!("cannot write {path}: {e}"))?;
    rusty_core::fs::written::note_written(path);
    Ok(path.to_string())
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod tests;
