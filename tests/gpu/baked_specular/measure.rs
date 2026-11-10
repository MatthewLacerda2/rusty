//! Comparing two shots' luminance, pixel by pixel.

use super::SIZE;

/// Where `a` exceeds `b` most, as (x, y, by how much).
pub(in crate::gpu) fn peak(a: &[f32], b: &[f32]) -> (u32, u32, f32) {
    let diffs = a.iter().zip(b).map(|(a, b)| a - b).enumerate();
    let (i, d) = diffs.fold(
        (0, f32::MIN),
        |best, (i, d)| if d > best.1 { (i, d) } else { best },
    );
    (i as u32 % SIZE, i as u32 / SIZE, d)
}

/// The most `a` and `b` differ by at any pixel.
pub(in crate::gpu) fn most(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max)
}

/// How much `a` and `b` differ, on average over the frame.
pub(in crate::gpu) fn mean_gap(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(a, b)| (a - b).abs()).sum::<f32>() / a.len() as f32
}
