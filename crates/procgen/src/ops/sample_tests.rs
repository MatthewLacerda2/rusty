use super::*;
use crate::image_buf::Size;

/// A one-texel checker: the map one bilinear tap aliases worst.
fn fine_checker(n: u32) -> Image {
    let mut img = Image::new(n);
    let w = n as usize;
    img.map_indexed(|i, _| {
        let c = ((i % w + i / w) % 2) as f32;
        [c, c, c, 1.0]
    });
    img
}

fn uv(size: impl Into<Size>, f: impl Fn(f32, f32) -> (f32, f32) + Sync) -> Image {
    Image::fill_uv(size, |u, v| {
        let (a, b) = f(u, v);
        [a, b, 0.0, 1.0]
    })
}

#[test]
fn identity_coordinates_read_the_map_back_exactly() {
    let src = Image::fill_uv(Size::new(8, 4), |u, v| [u, v * v, 0.25, 1.0]);
    let coords = uv(Size::new(8, 4), |u, v| (u, v));
    assert_eq!(sample_at(&src, Coords::Packed(&coords)), src);
    let (u, v) = (
        uv(Size::new(8, 4), |u, _| (u, 0.0)),
        uv(Size::new(8, 4), |_, v| (v, 0.0)),
    );
    assert_eq!(sample_at(&src, Coords::Split(&u, &v)), src);
}

#[test]
fn a_seam_in_the_coordinates_does_not_blur_along_it() {
    // Coordinates shifted half a tile wrap from 1 to 0 mid-image; every pixel, the
    // ones beside the jump included, still reads its one texel.
    let src = Image::fill_uv(8, |u, v| [u, v, 0.0, 1.0]);
    let coords = uv(8, |u, v| ((u + 0.5).fract(), v));
    let out = sample_at(&src, Coords::Packed(&coords));
    for y in 0..8 {
        for x in 0..8 {
            assert_eq!(
                out.get_wrapped(x, y),
                src.get_wrapped(x + 4, y),
                "({x}, {y})"
            );
        }
    }
}

#[test]
fn a_shrunk_map_reads_its_average_not_one_tap() {
    // Four repeats of a one-texel checker per output pixel: one tap reads 0 or 1,
    // the footprint's mean is grey.
    let src = fine_checker(32);
    let coords = uv(32, |u, v| (u * 4.0, v * 4.0));
    let out = sample_at(&src, Coords::Packed(&coords));
    for p in out.pixels() {
        assert!((p[0] - 0.5).abs() < 1e-5, "{p:?}");
    }
}

#[test]
fn a_flat_or_broken_footprint_reads_level_zero() {
    assert_eq!(lod(0.0), 0.0);
    assert_eq!(lod(f32::NAN), 0.0);
    assert_eq!(lod(f32::INFINITY), 0.0);
    assert_eq!(lod(4.0), 2.0);
    // Coordinates that are NaN read a finite pixel, as bilinear does.
    let coords = Image::filled(4, [f32::NAN, f32::INFINITY, 0.0, 1.0]);
    let out = sample_at(&fine_checker(4), Coords::Packed(&coords));
    assert!(out.pixels().iter().flatten().all(|c| c.is_finite()));
}

#[test]
fn each_mip_level_is_the_box_mean_of_the_last() {
    let src = Image::fill_uv(Size::new(8, 2), |u, v| [u, v, 0.0, 1.0]);
    let chain = mips(&src);
    let sizes: Vec<Size> = chain.iter().map(Image::size).collect();
    assert_eq!(sizes, [Size::new(4, 1), Size::new(2, 1), Size::new(1, 1)]);
    let mean = |x: i64| (src.get_wrapped(2 * x, 0)[0] + src.get_wrapped(2 * x + 1, 0)[0]) / 2.0;
    for x in 0..4 {
        assert!((chain[0].get_wrapped(x, 0)[0] - mean(x)).abs() < 1e-6);
        assert!((chain[0].get_wrapped(x, 0)[1] - 0.5).abs() < 1e-6);
    }
    assert!((chain[2].pixels()[0][0] - 0.5).abs() < 1e-6);
    // Level 0 is the source itself, never copied: the chain is under a third of it.
    let bytes = |c: &[Image]| c.iter().map(|l| l.pixels().len()).sum::<usize>();
    let big = mips(&Image::new(64));
    assert!(3 * bytes(&big) <= 64 * 64, "{}", bytes(&big));
    assert!(mips(&Image::new(1)).is_empty());
}
