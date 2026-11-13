//! Unity's 2D strip LUT: `N` slices of `N × N` laid side by side, `N² × N` pixels
//! (`256×16`, `1024×32`). Slice `b` holds blue `b`; within it red grows to the right
//! and green upwards, Unity's texture origin being the bottom-left: the image's
//! bottom row is green `0`.

use super::Lut;

/// The table an 8-bit RGBA strip of `width × height` holds.
pub(super) fn from_rgba8(width: u32, height: u32, rgba: &[u8]) -> Result<Lut, String> {
    // `fits` first: past it, `n * n` cannot overflow (a 1×70000 image).
    let n = height;
    if !Lut::fits(n) || width != n * n {
        let (lo, hi) = (Lut::MIN_SIZE, Lut::MAX_SIZE);
        return Err(format!(
            "{width}×{height} is not a LUT strip (N²×N, N {lo}–{hi}: 256×16, 1024×32)"
        ));
    }
    let (n, w) = (n as usize, width as usize);
    if rgba.len() < w * n * 4 {
        return Err(format!(
            "{} bytes is short of {width}×{height} RGBA",
            rgba.len()
        ));
    }
    let mut texels = Vec::with_capacity(n.pow(3));
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let (x, y) = (b * n + r, n - 1 - g);
                let p = &rgba[(y * w + x) * 4..][..3];
                texels.push([p[0], p[1], p[2]].map(|v| f32::from(v) / 255.0));
            }
        }
    }
    Ok(Lut {
        size: n as u32,
        domain: [[0.0; 3], [1.0; 3]],
        texels: texels.into_iter().map(|[r, g, b]| [r, g, b, 1.0]).collect(),
    })
}
