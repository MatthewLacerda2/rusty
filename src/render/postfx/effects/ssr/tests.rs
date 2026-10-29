//! The SSR quality's march (#1521), off the GPU.

use super::march;

/// Low marches fewer steps than Medium, Medium than High, High than Ultra, and each
/// reaches further than the one below it.
#[test]
fn each_quality_marches_more_steps_and_reaches_further() {
    let levels = ["Low", "Medium", "High", "Ultra"].map(march);
    for pair in levels.windows(2) {
        let (lo, hi) = (pair[0], pair[1]);
        assert!(lo.steps < hi.steps, "{lo:?} vs {hi:?}");
        assert!(
            lo.steps as f32 * lo.step < hi.steps as f32 * hi.step,
            "{lo:?} vs {hi:?}"
        );
    }
    assert_eq!(levels[0].steps, 16, "HDRP's Low");
    assert_eq!(levels[2].steps, 64, "HDRP's High");
}

#[test]
fn an_unknown_quality_marches_as_high() {
    assert_eq!(march("Cinematic"), march("High"));
}
