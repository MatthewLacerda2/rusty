use super::gain::Ballistics;
use crate::level::intersample::{Channel, TAPS};
use crate::stereo::Stereo;

/// −1 dBTP, 10^(−1/20): a decibel of headroom for a lossy codec's reconstruction.
const CEILING: f32 = 0.891_251;
/// Seconds; also how far a peak reaches back, which `song::excerpt` renders past a window.
pub(crate) const LOOKAHEAD: f32 = 0.002;
const RELEASE: f32 = 0.06;

pub(crate) fn apply(buf: &mut Stereo, rate: f32) {
    let gain = Ballistics::new(LOOKAHEAD, RELEASE, rate).smooth(&required_gain(buf));
    buf.each(|channel| {
        for (s, g) in channel.iter_mut().zip(&gain) {
            *s = (*s * g).clamp(-1.0, 1.0);
        }
    });
}

pub(crate) fn apply_looped(buf: &mut Stereo, rate: f32) {
    let frames = buf.frames();
    if frames == 0 {
        return;
    }
    let reach = Ballistics::new(LOOKAHEAD, RELEASE, rate).reach_frames() + 2 * TAPS + 2;
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

fn required_gain(buf: &mut Stereo) -> Vec<f32> {
    frame_peaks(buf)
        .into_iter()
        .map(|peak| (CEILING / peak).min(1.0))
        .collect()
}

fn frame_peaks(buf: &mut Stereo) -> Vec<f32> {
    sanitise(buf);
    // Each side reconstructed alone: an overshoot on either is real.
    let (left, right) = (Channel::mono(&buf.l), Channel::mono(&buf.r));
    (0..buf.frames())
        .map(|frame| left.peak_from(frame).max(right.peak_from(frame)) as f32)
        .collect()
}

/// Runs first: reconstruction reads `TAPS` samples either side, so a NaN poisons its neighbours.
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

    fn intersample_tone(n: usize, amplitude: f32) -> Vec<f32> {
        (0..n)
            .map(|i| amplitude * (TAU * i as f32 / 4.0 + std::f32::consts::FRAC_PI_4).sin())
            .collect()
    }

    fn measured(buf: &Stereo) -> crate::level::Loudness {
        let mut meter = crate::level::Meter::new(crate::stereo::CHANNELS);
        meter.feed(&buf.interleaved());
        meter.finish()
    }

    #[test]
    fn the_waveform_between_the_samples_is_held_down_too() {
        let buf = limited(Stereo::centred(intersample_tone(4410, 3.0)));
        let loud = measured(&buf);
        assert!(!loud.is_clipping(), "the meter still calls this clipping");
        let true_peak = loud.true_peak_dbfs.expect("it is not silent");
        assert!(true_peak < -0.9, "true peak {true_peak} dBFS");
        // The samples sit well under the ceiling: it holds the waveform, not the samples.
        assert!(peak(&buf.l) < CEILING * 0.8, "peaked at {}", peak(&buf.l));
    }

    #[test]
    fn a_hot_signal_is_brought_under_the_ceiling() {
        let buf = limited(Stereo::centred(sine(44_100, 220.0, 3.0)));
        assert!(peak(&buf.l) <= CEILING + 1e-6, "peaked at {}", peak(&buf.l));
        assert!(peak(&buf.l) > 0.5, "but it is still a loud signal");
        assert_eq!(buf.l, buf.r, "and both sides took the same treatment");
    }

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
        // Silence, then one enormous spike: the samples just before it are quiet, so the ducking is
        // only visible as the spike itself being tamed.
        let mut mono = vec![0.0; 4410];
        mono[2205] = 10.0;
        let buf = limited(Stereo::centred(mono));
        assert!(buf.l[2205] <= CEILING + 1e-6);
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

    #[test]
    fn silence_stays_silent_rather_than_dividing_into_nothing() {
        let quiet = limited(Stereo::silence(512));
        assert!(quiet.l.iter().chain(&quiet.r).all(|s| *s == 0.0));
        assert!(required_gain(&mut Stereo::silence(512))
            .iter()
            .all(|g| *g == 1.0));
    }
}
