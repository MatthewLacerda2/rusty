//! src/procgen/recipe/extent.rs — a recipe's pixel extent (#1493): the square
//! `resolution` shorthand, or `size = [w, h]` for a non-square bake.
//!
//! The recipe's domain is the unit square in relative units whatever the extent
//! (see [`crate::procgen::image_buf`]): a `[2048, 512]` trim sheet samples the same
//! tile as a square bake with four times the pixels across, so its pattern
//! stretches, and an author who wants square bricks on it sets `cols = 4 × rows`.
//! Every periodic count stays whole, so the bake still tiles on both axes.

use serde::{Deserialize, Deserializer};

use super::TextureRecipe;
use crate::procgen::image_buf::Size;

/// Parse `size = [w, h]`, refusing a side that is not a positive power of two.
pub(super) fn power_of_two_sides<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<[u32; 2]>, D::Error> {
    let sides = Option::<[u32; 2]>::deserialize(d)?;
    if let Some(bad) = sides.iter().flatten().find(|s| !s.is_power_of_two()) {
        return Err(serde::de::Error::custom(format!(
            "size side {bad} is not a positive power of two"
        )));
    }
    Ok(sides)
}

impl TextureRecipe {
    /// The extent the recipe bakes at: `size` when set, else `resolution` squared.
    pub fn extent(&self) -> Size {
        match self.size {
            Some([w, h]) => Size::new(w, h),
            None => Size::from(self.resolution),
        }
    }

    /// Bake at `resolution` from now on. A square recipe takes it as its side. A
    /// `size` recipe keeps its aspect (#1493): both sides scale by one power of two
    /// so the longer one equals `resolution`, `[2048, 512]` at 256 becoming
    /// `[256, 64]`. That needs `resolution` to be a power of two and leaves the
    /// shorter side at least one pixel; anything else is refused and the recipe is
    /// unchanged.
    pub fn set_resolution(&mut self, resolution: u32) -> Result<(), String> {
        let Some([w, h]) = self.size else {
            self.resolution = resolution;
            return Ok(());
        };
        if !resolution.is_power_of_two() {
            return Err(format!(
                "resolution {resolution} is not a power of two, which a `size` recipe needs to keep its aspect"
            ));
        }
        let longer = w.max(h);
        // Sides are powers of two, so the ratio is exact either way.
        let scale = |s: u32| match resolution >= longer {
            true => s.checked_mul(resolution / longer),
            false => Some(s / (longer / resolution)).filter(|&s| s > 0),
        };
        let (Some(nw), Some(nh)) = (scale(w), scale(h)) else {
            return Err(format!(
                "size [{w}, {h}] can't keep its aspect with its longer side at {resolution}"
            ));
        };
        self.size = Some([nw, nh]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sized(size: [u32; 2]) -> TextureRecipe {
        TextureRecipe::new(0, vec![]).with_size(size)
    }

    #[test]
    fn a_square_recipe_takes_the_resolution_as_its_side() {
        let mut r = TextureRecipe::new(64, vec![]);
        r.set_resolution(100).unwrap();
        assert_eq!(r.extent(), Size::new(100, 100));
    }

    #[test]
    fn a_sized_recipe_keeps_its_aspect_with_the_longer_side_at_the_resolution() {
        for (size, res, want) in [
            ([2048, 512], 256, [256, 64]),
            ([512, 2048], 256, [64, 256]),
            ([64, 16], 1024, [1024, 256]),
            ([32, 32], 8, [8, 8]),
        ] {
            let mut r = sized(size);
            r.set_resolution(res).unwrap();
            assert_eq!(r.size, Some(want), "{size:?} at {res}");
        }
    }

    #[test]
    fn a_rescale_that_would_lose_the_aspect_is_refused_and_changes_nothing() {
        for res in [300, 2, 0] {
            let mut r = sized([2048, 512]);
            assert!(r.set_resolution(res).is_err(), "{res}");
            assert_eq!(r.size, Some([2048, 512]));
        }
    }

    #[test]
    fn a_side_that_is_not_a_power_of_two_is_a_parse_error() {
        let parse = |size: &str| {
            let json = format!(r#"{{ "size": {size}, "nodes": [] }}"#);
            serde_json::from_str::<TextureRecipe>(&json).map(|r| r.extent())
        };
        assert_eq!(parse("[256, 64]").unwrap(), Size::new(256, 64));
        for bad in ["[256, 48]", "[0, 64]", "[300, 300]"] {
            let err = parse(bad).unwrap_err().to_string();
            assert!(err.contains("not a positive power of two"), "{bad}: {err}");
        }
    }
}
