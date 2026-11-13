//! What a normal map's mips lose (#1702, Toksvig 2005): a level averages the
//! normals of the texels it covers, and where they disagreed the average is
//! shorter than 1. That shortening is a spread of normals inside one texel, which
//! a GGX lobe narrower than it aliases (a busy normal map's highlight sparkles at
//! a distance), so the forward shader widens the lobe by it
//! (`forward::specular_aa::texture_kernel`).
//!
//! A map whose texels keep z (uncompressed RGBA8, BC7) gives the shader the
//! average itself, at the very level it samples. A BC5 map keeps only x and y, the
//! shader rebuilds z, and the length is lost; so the importer measures it while it
//! still has z, and stores each level's mean [`kernel`] in the cache: a
//! [`VarianceTable`], which the shader reads at the level it samples
//! (`forward::specular_aa::table_kernel`). The table is per level, not per texel:
//! a map's busy and smooth regions widen alike at one distance, by their mean.

use image::Rgba32FImage;

/// How many levels below the base a [`VarianceTable`] holds: a 32768² map's.
pub const VARIANCE_LEVELS: usize = 16;

/// Each mip level's mean [`kernel`], level 1 at index 0 (the base level's is 0:
/// its texels are whole normals), in 255ths of [`KERNEL_MAX`]. Levels past a
/// map's last repeat it; all zero for a map that is not BC5, or has no mips.
pub type VarianceTable = [u8; VARIANCE_LEVELS];

/// The most a texel's spread adds to α² (squared GGX roughness): HDRP Lit's
/// `_SpecularAAThreshold` squared, `specular_aa.wgsl`'s `THRESHOLD`.
pub const KERNEL_MAX: f32 = 0.04;

/// The length an 8-bit encoding can lose from a whole normal (2/255 in [-1,1]):
/// a shortening this small is rounding, not spread, and widens nothing.
pub const QUANTUM: f32 = 2.0 / 255.0;

/// What a texel whose averaged normal is `length` long adds to α²: HDRP's
/// `TextureNormalVariance` (the vMF fit of the normals, `κ = (3r − r³)/(1 − r²)`,
/// variance `1/(4κ)`), doubled and capped as `TextureNormalFiltering` does.
/// `specular_aa::texture_kernel` is the same sum.
pub fn kernel(length: f32) -> f32 {
    let r = (length + QUANTUM).min(1.0);
    let r2 = r * r;
    (0.5 * (1.0 - r2) / (r * (3.0 - r2)).max(1e-4)).min(KERNEL_MAX)
}

/// `level`'s normals as the shader reads a BC5 map's (`forward::maps::unpack_normal`):
/// x and y as stored, z rebuilt, so each is whole whatever the source's blue held.
pub(super) fn as_read(level: &Rgba32FImage) -> Rgba32FImage {
    Rgba32FImage::from_fn(level.width(), level.height(), |x, y| {
        let p = level.get_pixel(x, y);
        let (nx, ny) = (p[0] * 2.0 - 1.0, p[1] * 2.0 - 1.0);
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        image::Rgba([p[0], p[1], nz * 0.5 + 0.5, 1.0])
    })
}

/// The mean [`kernel`] of a level's texels, its rgb a normal stored raw in [0,1].
pub(super) fn level_mean(level: &Rgba32FImage) -> f32 {
    let sum: f64 = level
        .pixels()
        .map(|p| {
            let n = [p[0], p[1], p[2]].map(|c| c * 2.0 - 1.0);
            f64::from(kernel((n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt()))
        })
        .sum();
    (sum / f64::from((level.width() * level.height()).max(1))) as f32
}

/// The table of `means`, levels 1 on ([`VarianceTable`]).
pub(super) fn table(means: &[f32]) -> VarianceTable {
    let mut table = [0; VARIANCE_LEVELS];
    for (i, entry) in table.iter_mut().enumerate() {
        let mean = means.get(i).or(means.last()).copied().unwrap_or(0.0);
        *entry = (mean / KERNEL_MAX * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::{set_texture, TextureSettings, TextureType};
    use crate::texture_import::{load, refresh, Compressed, Texels, TextureKind};
    use image::{Rgba, RgbaImage};
    use std::path::Path;

    #[test]
    fn a_whole_or_rounded_normal_adds_nothing_and_a_short_one_adds_up_to_the_cap() {
        assert_eq!(kernel(1.0), 0.0);
        assert_eq!(kernel(1.0 - QUANTUM), 0.0, "8-bit rounding is not spread");
        assert_eq!(kernel(1.2), 0.0);
        let mild = kernel(0.98);
        assert!(mild > 0.005 && mild < 0.007, "{mild}");
        assert_eq!(kernel(0.5), KERNEL_MAX);
        assert_eq!(kernel(0.0), KERNEL_MAX);
    }

    #[test]
    fn the_shader_sums_with_the_same_constants() {
        let path = rusty_core::project::checkout().join("engine/shaders/forward/specular_aa.wgsl");
        let wgsl = std::fs::read_to_string(path).unwrap();
        assert!(wgsl.contains("const THRESHOLD: f32 = 0.2;"));
        assert!(wgsl.contains("const QUANTUM: f32 = 2.0 / 255.0;"));
        assert!((KERNEL_MAX - 0.2f32 * 0.2).abs() < 1e-7);
    }

    #[test]
    fn the_table_quantizes_each_level_and_repeats_the_last() {
        let t = table(&[0.0, KERNEL_MAX / 2.0, KERNEL_MAX]);
        assert_eq!(&t[..4], &[0, 128, 255, 255]);
        assert_eq!(t[VARIANCE_LEVELS - 1], 255);
        assert_eq!(table(&[]), [0; VARIANCE_LEVELS]);
    }

    #[test]
    fn a_flat_level_has_no_spread_and_a_mixed_one_has() {
        let flat = Rgba32FImage::from_pixel(4, 4, image::Rgba([0.5, 0.5, 1.0, 1.0]));
        assert_eq!(level_mean(&flat), 0.0);
        // Two normals 90° apart averaged: length ½√2.
        let mixed = Rgba32FImage::from_pixel(2, 2, image::Rgba([0.75, 0.5, 0.75, 1.0]));
        assert_eq!(level_mean(&mixed), KERNEL_MAX);
    }

    /// The encoder halves the z-rebuilt image in place of the stored one for BC5,
    /// which keeps x and y only: `half` must leave those as halving the stored
    /// image would, whatever its blue and alpha held.
    #[test]
    fn rebuilding_z_changes_no_x_or_y_of_any_mip() {
        let stored = Rgba32FImage::from_fn(8, 8, |x, y| {
            let v = |k: u32| ((x * 7 + y * 13 + k * 5) % 11) as f32 / 10.0;
            image::Rgba([v(0), v(1), v(2), v(3)])
        });
        let (mut a, mut b) = (stored.clone(), as_read(&stored));
        for _ in 0..3 {
            (a, b) = (
                super::super::shrink::half(&a),
                super::super::shrink::half(&b),
            );
            let rg = |i: &Rgba32FImage| i.pixels().flat_map(|p| [p[0], p[1]]).collect::<Vec<_>>();
            assert_eq!(rg(&a), rg(&b));
        }
    }

    #[test]
    fn a_bc5_cache_keeps_what_its_mips_lost_and_a_flat_or_colour_one_nothing() {
        let dir = rusty_test_support::temp::dir();
        let normal = typed_png(&dir, "bumps.png", TextureType::NormalMap, |x, _| {
            let tilt = if x % 2 == 0 { 64 } else { 192 };
            Rgba([tilt, 128, 238, 255])
        });
        let t = normal.variance;
        assert!(t[0] > 0 && t[1..].iter().all(|&v| v == t[0]), "{t:?}");
        let flat = typed_png(&dir, "flat.png", TextureType::NormalMap, |_, _| {
            Rgba([128, 128, 255, 255])
        });
        assert_eq!(flat.variance, [0; 16]);
        let colour = typed_png(&dir, "bumps_c.png", TextureType::Default, |x, _| {
            Rgba([(x * 64) as u8, 0, 0, 255])
        });
        assert_eq!(colour.variance, [0; 16]);
    }

    /// A refreshed 16² image of `texel` typed `ty`, read back from its cache.
    fn typed_png(
        dir: &Path,
        name: &str,
        ty: TextureType,
        texel: fn(u32, u32) -> Rgba<u8>,
    ) -> Compressed {
        let png = dir.join(name);
        RgbaImage::from_fn(16, 16, texel).save(&png).unwrap();
        set_texture(
            &png,
            TextureSettings {
                texture_type: ty,
                ..Default::default()
            },
        )
        .unwrap();
        refresh(&png, "").unwrap();
        let settings = crate::sidecar::texture(&png);
        let kind =
            [TextureKind::Colour, TextureKind::Normal][usize::from(ty == TextureType::NormalMap)];
        match load(&png, &settings, kind, true).unwrap().texels {
            Texels::Blocks(blocks) => blocks,
            Texels::Image(_) => panic!("{name} not cached"),
        }
    }
}
