//! src/procgen/recipe/color.rs — a `color` param: a linear RGBA array, or sRGB hex
//! (#1487).
//!
//! An array is taken as linear, the space the op chain works in. A `"#rrggbb"` /
//! `"#rrggbbaa"` string is sRGB, the way colours are picked from a reference, and is
//! decoded to linear on parse (alpha stays linear), like Blender's hex field. Either
//! way the recipe holds the linear array, so `Texture.ToJson` writes that.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::core::color::srgb_to_linear;

/// `color` on `constant` and on a ramp stop.
pub fn color<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 4], D::Error> {
    named(d, "color")
}

/// `color_a` on `checker`.
pub fn color_a<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 4], D::Error> {
    named(d, "color_a")
}

/// `color_b` on `checker`.
pub fn color_b<'de, D: Deserializer<'de>>(d: D) -> Result<[f32; 4], D::Error> {
    named(d, "color_b")
}

/// Parse param `name`, naming it in the error: serde reports a custom error without
/// the field it came from.
fn named<'de, D: Deserializer<'de>>(d: D, name: &str) -> Result<[f32; 4], D::Error> {
    match Value::deserialize(d)? {
        Value::String(hex) => from_hex(&hex).ok_or_else(|| {
            D::Error::custom(format!(
                "{name}: {hex:?} is not \"#rrggbb\" or \"#rrggbbaa\""
            ))
        }),
        array => serde_json::from_value(array).map_err(|e| {
            D::Error::custom(format!("{name}: {e} (a linear RGBA array or sRGB hex)"))
        }),
    }
}

/// Decode `"#rrggbb"` / `"#rrggbbaa"`: RGB sRGB → linear, alpha as written / 255.
pub fn from_hex(hex: &str) -> Option<[f32; 4]> {
    let digits = hex.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).ok();
    let alpha = if digits.len() == 8 { byte(3)? } else { 255 };
    Some([
        srgb_to_linear(byte(0)?),
        srgb_to_linear(byte(1)?),
        srgb_to_linear(byte(2)?),
        f32::from(alpha) / 255.0,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mid_grey_hex_decodes_to_linear() {
        let [r, g, b, a] = from_hex("#808080").unwrap();
        assert!((r - 0.2158).abs() < 1e-3, "{r}");
        assert_eq!((r, r, 1.0), (g, b, a));
    }

    #[test]
    fn hex_alpha_is_linear() {
        assert_eq!(from_hex("#ffffff80").unwrap()[3], 128.0 / 255.0);
        assert_eq!(from_hex("#000000").unwrap(), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn malformed_hex_is_refused() {
        for bad in ["808080", "#80808", "#80808g", "#8080808080", "#ééé"] {
            assert_eq!(from_hex(bad), None, "{bad}");
        }
    }
}
