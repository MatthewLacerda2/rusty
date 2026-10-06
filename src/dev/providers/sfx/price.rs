//! What a sound effect is estimated to cost (#388): rusty's own rate, because the
//! pinned scorsese has no sound-effects table.
//!
//! **Unverified, and deliberately high.** On the day it was checked the vendor's
//! pricing page was out of reach, and the sources that were reachable disagree:
//! 11 credits per second on the API, 40 on the website, flat figures per generation
//! elsewhere (#388 lists them). This takes the highest per-second figure, 40
//! credits, at the rate scorsese's speech table implies (1 credit ≈ 0.01¢), so it can
//! only over-estimate and the budget can never be crossed by a guess. Re-read
//! <https://elevenlabs.io/pricing/api> and correct [`MILLS_PER_SECOND`] and
//! [`CHECKED`] together.

/// The day the rate below was last checked.
pub const CHECKED: &str = "2026-10-06";

/// Tenths of a US cent per second of requested duration: 40 credits a second.
pub const MILLS_PER_SECOND: u64 = 4;

/// What `seconds` of effect is estimated to cost, in US cents, rounded up so a run
/// can never creep past the budget. Billed on the length asked for, not the words.
pub fn estimate_cents(seconds: f64) -> u64 {
    // Whole milliseconds first, so the arithmetic below is exact integers. The
    // brief's bounds (0.5 to 30) keep this small and positive.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let millis = (seconds * 1000.0).ceil() as u64;
    (millis * MILLS_PER_SECOND).div_ceil(10_000)
}
