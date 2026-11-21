//! the peak limiter every bake passes through.
//!
//! Not an effect a recipe chooses: a bake **must never clip**. Detuned oscillators
//! drift in and out of phase, a resonant filter can double a peak, and summed notes
//! add up — any of which can push a buffer past ±1, where 16-bit quantization turns
//! the overshoot into a click. So the last thing a bake does is limit.
//!
//! **The peak it is held to is the *true* peak**, not the loudest sample. A sample
//! is a point on a band-limited curve, and the curve between two samples routinely
//! exceeds both of them — so a buffer whose every sample sat under the old ceiling
//! of 0.98 could still pass full scale the moment a converter, `scorsese-render`'s
//! resampler or a delivery codec reconstructed it. That is not a tolerance quibble:
//! it is a promise this module made and did not keep, on files
//! [`crate::level::meter`] was already reporting as clipping. The required gain is
//! therefore computed from [`crate::level::intersample`] — the same reconstruction
//! the meter measures with, so the guarantee and the report cannot drift apart
//! again.
//!
//! Because we render offline we can do this properly, with **lookahead**: compute
//! the gain each sample needs, then smooth it so the gain is already down *before*
//! a peak arrives (a short attack ramp instead of an instant duck) and recovers
//! over the release in decibels. Every pass of [`super::gain`] only ever lowers the
//! gain, so the ceiling is guaranteed, and a signal that never approaches it is
//! left untouched.
//!
//! **The two channels are linked**, which is the one thing a stereo limiter has
//! to get right. There is a single gain track, computed from whichever side is
//! louder at each sample and applied to both. Limiting each channel on its own
//! would be a fader that moved one side and not the other every time a peak
//! landed off-centre — a centred vocal would drift left whenever the hats on
//! the right got loud, and the whole image would breathe sideways. Linking
//! costs a `max`; not linking costs the stereo image, and a peak on one side
//! genuinely *is* a moment when the mix is too loud.

use super::gain::Ballistics;
use crate::level::intersample::{Channel, TAPS};
use crate::stereo::Stereo;

/// The true peak the output is held under: **−1.0 dBTP**.
///
/// Chosen rather than inherited. A decibel of headroom is the broadcast
/// convention for material that will be lossily encoded, and every bake here is
/// on its way into a video that will be: an MP3 or AAC encoder reconstructs the
/// waveform its own way and lands a few tenths of a decibel either side of what
/// the source measured, so a master pressed against full scale arrives clipped
/// on the far end however carefully it was limited. The old ceiling of 0.98
/// (≈ −0.18 dBFS) left a codec nothing.
///
/// The same decibel pays for the approximation below. The gain track is one
/// number per sample-frame while an overshoot lives *between* frames, so the
/// gain the reconstruction actually sees around a peak is a hair higher than
/// the one that peak asked for. That error is fractions of a percent against
/// 10% of headroom — but it is why the ceiling is a considered number and not
/// the largest one that fits.
const CEILING: f32 = 0.891_251; // 10^(-1/20)
/// Lookahead / attack ramp, in seconds — long enough to duck without a click.
///
/// Also the distance a peak reaches **backwards**: the gain may fall no faster
/// than this ramp, so nothing further ahead than it can change a sample here.
/// [`crate::song::excerpt`] renders that far past a window for exactly that
/// reason, which is why the number is published rather than private.
pub(crate) const LOOKAHEAD: f32 = 0.002;
/// Recovery time constant, in seconds. Slower than the attack, or the gain
/// pumps audibly.
const RELEASE: f32 = 0.06;

/// Limit `buf` in place so no sample exceeds the ceiling. Non-finite samples (from
/// a pathological patch) are flushed to silence rather than written to the file.
pub(crate) fn apply(buf: &mut Stereo, rate: f32) {
    let gain = Ballistics::new(LOOKAHEAD, RELEASE, rate).smooth(&required_gain(buf));
    buf.each(|channel| {
        for (s, g) in channel.iter_mut().zip(&gain) {
            *s = (*s * g).clamp(-1.0, 1.0);
        }
    });
}

/// [`apply`], to a buffer that is going to be played **round and round** — a
/// loop whose last sample is followed by its first.
///
/// [`apply`] reads a buffer as a line with silence off both ends, and that is
/// exactly wrong at a loop point: a loud first bar would be reconstructed as
/// an edge rising out of nothing (a false overshoot, so a false duck), and a
/// peak in the first bar would not pull the gain down in the last one, which
/// is where its lookahead has to start. Either puts a step in the gain on the
/// seam — the one place a looping file promises has none.
///
/// So the buffer is limited with enough of its own other end wrapped on to
/// each side for every stage to see what it would see mid-loop, and only the
/// middle is kept. How much is enough is how far any one frame's gain can
/// reach ([`Ballistics::reach_frames`]: the hold and the release forwards,
/// the lookahead backwards), and the reconstruction's taps under both. That is **exactly** the result of
/// limiting the loop played three times and keeping the middle pass, which
/// is what the test below holds it to — and it costs a few thousand frames
/// rather than two whole passes.
pub(crate) fn apply_looped(buf: &mut Stereo, rate: f32) {
    let frames = buf.frames();
    if frames == 0 {
        return;
    }
    let reach = Ballistics::new(LOOKAHEAD, RELEASE, rate).reach_frames() + 2 * TAPS + 2;
    // Index `reach` of the circle is frame 0 of the loop; `frames * reach`
    // is a multiple of `frames` at least `reach` long, so nothing underflows.
    let circle = |channel: &[f32]| -> Vec<f32> {
        (0..frames + 2 * reach)
            .map(|index| channel[(index + frames * reach - reach) % frames])
            .collect()
    };
    let mut around = Stereo {
        l: circle(&buf.l),
        r: circle(&buf.r),
    };
    apply(&mut around, rate);
    around.cut(reach, reach + frames);
    *buf = around;
}

/// The instantaneous gain each sample-frame needs to sit under the ceiling —
/// 1.0 wherever the signal is already quiet enough.
///
/// Written as a ratio clamped at unity rather than as a test and a ratio,
/// because the test had no edge to get wrong: at a peak of exactly the ceiling
/// the ratio is exactly `1.0`, so `>` and `>=` were the same function, and a
/// silent frame divides to an infinity the clamp takes back to unity. A branch
/// no input can tell apart from its alternative is complexity worth deleting
/// rather than complexity worth explaining.
fn required_gain(buf: &mut Stereo) -> Vec<f32> {
    frame_peaks(buf)
        .into_iter()
        .map(|peak| (CEILING / peak).min(1.0))
        .collect()
}

/// The loudest the waveform gets over each sample-frame — the sample itself
/// and the reconstruction up to the next one, which
/// [`Channel::peak_from`](crate::level::intersample::Channel::peak_from)
/// covers in one number. Sanitizes `buf` first.
///
/// One number per frame, from the louder of the two channels: that is what
/// links them. Reconstructing each channel separately and taking the maximum,
/// rather than reconstructing some sum of them, because an overshoot on either
/// side is a real one — the two are played through different speakers.
fn frame_peaks(buf: &mut Stereo) -> Vec<f32> {
    sanitise(buf);
    let (left, right) = (Channel::mono(&buf.l), Channel::mono(&buf.r));
    (0..buf.frames())
        .map(|frame| left.peak_from(frame).max(right.peak_from(frame)) as f32)
        .collect()
}

/// Flushes every non-finite sample to silence — a `NaN` that reached the file
/// would be a click, and a `NaN` left in the gain arithmetic would silence
/// everything around it.
///
/// Before anything is measured rather than as it is measured, because the
/// reconstruction reads eight samples either side: one `NaN` left in the buffer
/// would poison every frame near it, not merely its own.
fn sanitise(buf: &mut Stereo) {
    buf.each(|channel| {
        for sample in channel {
            if !sample.is_finite() {
                *sample = 0.0;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    fn sine(n: usize, hz: f32, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (TAU * hz * i as f32 / 44_100.0).sin())
            .collect()
    }

    fn limited(buf: Stereo) -> Stereo {
        let mut buf = buf;
        apply(&mut buf, 44_100.0);
        buf
    }

    /// A tone at a quarter of the sample rate, offset so every sample lands
    /// halfway up the slope: the samples read `amplitude / √2` and the
    /// waveform between them reaches `amplitude`. Hats, a bright saw and any
    /// sharp transient all carry energy up here.
    fn intersample_tone(n: usize, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (TAU * i as f32 / 4.0 + std::f32::consts::FRAC_PI_4).sin())
            .collect()
    }

    /// The bake put through the same meter `scorsese level` prints from.
    fn measured(buf: &Stereo) -> crate::level::Loudness {
        let mut meter = crate::level::Meter::new(crate::stereo::CHANNELS);
        meter.feed(&buf.interleaved());
        meter.finish()
    }

    /// The failure this module was fixed for, in its purest form: every sample
    /// under the old ceiling, and the waveform between them three decibels
    /// over full scale. The signal is real — hats and bright saws all carry
    /// energy this high — and it is the case a sample-peak limiter cannot see.
    #[test]
    fn the_waveform_between_the_samples_is_held_down_too() {
        let buf = limited(Stereo::centred(intersample_tone(4410, 3.0)));
        let loud = measured(&buf);
        assert!(!loud.is_clipping(), "the meter still calls this clipping");
        let true_peak = loud.true_peak_dbfs.expect("it is not silent");
        assert!(true_peak < -0.9, "true peak {true_peak} dBFS");
        // Which cost it real level: the samples now sit well under the
        // ceiling, because it is not the samples the ceiling is about.
        assert!(peak(&buf.l) < CEILING * 0.8, "peaked at {}", peak(&buf.l));
    }

    #[test]
    fn a_hot_signal_is_brought_under_the_ceiling() {
        let buf = limited(Stereo::centred(sine(44_100, 220.0, 3.0)));
        assert!(peak(&buf.l) <= CEILING + 1e-6, "peaked at {}", peak(&buf.l));
        assert!(peak(&buf.l) > 0.5, "but it is still a loud signal");
        assert_eq!(buf.l, buf.r, "and both sides took the same treatment");
    }

    /// A loop's seam is limited as if it were the middle of the music: what
    /// comes back is the middle pass of the loop limited three times over,
    /// sample for sample. Hot enough to be limited throughout, and a tone
    /// that does not fit the buffer a whole number of times, so the seam is a
    /// real edge that a line-reading limiter would treat differently.
    #[test]
    fn a_loop_is_limited_as_the_circle_it_is() {
        let tone = sine(22_050, 237.0, 2.5);
        let mut looped = Stereo::centred(tone.clone());
        apply_looped(&mut looped, 44_100.0);
        let thrice = limited(Stereo::centred(tone.repeat(3)));
        assert_eq!(looped.l[..], thrice.l[22_050..44_100]);
        assert_eq!(looped.r[..], thrice.r[22_050..44_100]);
        let line = limited(Stereo::centred(tone));
        assert_ne!(looped.l, line.l, "the seam is limited differently");
        assert_eq!(
            looped.l[5_000..17_000],
            line.l[5_000..17_000],
            "and only the seam: away from it the loop is any other music"
        );
    }

    /// The linking has to survive the change: an overshoot that exists only
    /// *between* one side's samples still ducks both, or the image lurches
    /// away from it exactly as it would for a sampled peak.
    #[test]
    fn an_overshoot_between_one_sides_samples_ducks_both() {
        let mut buf = Stereo {
            l: intersample_tone(4410, 1.2),
            r: sine(4410, 220.0, 0.4),
        };
        let before = peak(&buf.r);
        apply(&mut buf, 44_100.0);
        assert!(peak(&buf.l) < CEILING, "the loud side never reached it");
        let after = peak(&buf.r);
        assert!(
            after < before * 0.95,
            "the quiet side did not follow: {before} → {after}"
        );
        assert!(!measured(&buf).is_clipping());
    }

    #[test]
    fn a_quiet_signal_passes_through_untouched() {
        let original = Stereo::centred(sine(1000, 100.0, 0.5));
        assert_eq!(
            limited(original.clone()),
            original,
            "a limiter must be transparent below its ceiling"
        );
    }

    /// The reason the channels are linked: a peak on one side ducks both, so
    /// the balance between them is exactly what it was. Unlinked, the quiet
    /// side would keep its level and the image would lurch away from the loud
    /// one every time the limiter acted.
    #[test]
    fn a_peak_on_one_side_ducks_both_by_the_same_amount() {
        let mut buf = Stereo {
            l: sine(4410, 220.0, 3.0),
            r: sine(4410, 220.0, 0.5),
        };
        let before = peak(&buf.r);
        apply(&mut buf, 44_100.0);
        assert!(peak(&buf.l) <= CEILING + 1e-6, "the loud side is held");
        let after = peak(&buf.r);
        assert!(
            after < before * 0.9,
            "the quiet side did not follow: {before} → {after}"
        );
        let reduction = after / before;
        assert!(
            (reduction - CEILING / 3.0).abs() < 0.05,
            "the two sides took different gains ({reduction})"
        );
    }

    #[test]
    fn the_gain_is_already_down_when_a_lone_peak_arrives() {
        // Silence, then one enormous spike: the samples just before it are quiet,
        // so the ducking is only visible as the spike itself being tamed.
        let mut mono = vec![0.0; 4410];
        mono[2205] = 10.0;
        let buf = limited(Stereo::centred(mono));
        assert!(buf.l[2205] <= CEILING + 1e-6);
        // The gain ramps back over the release, not instantly.
        let gain = Ballistics::new(LOOKAHEAD, RELEASE, 44_100.0).smooth(&required_gain(
            &mut Stereo::centred(vec![0.0, 10.0, 0.0, 0.0]),
        ));
        assert!(gain[0] < 1.0, "ducked before the peak");
        assert!(gain[2] < 1.0, "and recovers gradually after it");
    }

    #[test]
    fn non_finite_samples_are_flushed_to_silence() {
        let buf = limited(Stereo {
            l: vec![f32::NAN, f32::INFINITY, 0.5, f32::NEG_INFINITY],
            r: vec![0.5, f32::NAN, f32::INFINITY, 0.25],
        });
        assert!(
            buf.l.iter().chain(&buf.r).all(|s| s.is_finite()),
            "got {buf:?}"
        );
        assert_eq!(buf.l[0], 0.0);
        assert_eq!(buf.r[1], 0.0);
    }

    #[test]
    fn an_empty_buffer_is_a_no_op() {
        assert!(limited(Stereo::silence(0)).is_empty());
    }

    /// Silence is the edge the gain is a division at: nothing over nothing is
    /// an infinity, and an infinity that reached the samples would make the
    /// whole file a `NaN` rather than a quiet passage.
    #[test]
    fn silence_stays_silent_rather_than_dividing_into_nothing() {
        let quiet = limited(Stereo::silence(512));
        assert!(quiet.l.iter().chain(&quiet.r).all(|s| *s == 0.0));
        assert!(
            required_gain(&mut Stereo::silence(512))
                .iter()
                .all(|g| *g == 1.0)
        );
    }
}
