//! trim — where a baked one-shot's file ends.

use crate::level::db_to_gain_f32;
use crate::stereo::Stereo;

/// dB below the sound's own peak: RT60, a thousandth of its amplitude, so the cut needs no fade.
pub(crate) const FLOOR_DB: f32 = -60.0;

pub(crate) fn to_floor(buf: &mut Stereo) {
    let loudest = |i: usize| {
        let (l, r) = buf.frame(i);
        l.abs().max(r.abs())
    };
    let peak = (0..buf.frames()).map(loudest).fold(0.0, f32::max);
    let floor = peak * db_to_gain_f32(FLOOR_DB);
    let last = (0..buf.frames())
        .rev()
        .find(|&i| peak > 0.0 && loudest(i) >= floor)
        .unwrap_or(0);
    buf.resize(last + 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(samples: &[f32]) -> Stereo {
        Stereo::centred(samples.to_vec())
    }

    fn ratio() -> f32 {
        db_to_gain_f32(FLOOR_DB)
    }

    #[test]
    fn a_tail_below_the_floor_is_dropped() {
        let quiet = ratio() * 0.5;
        let mut buf = signal(&[1.0, 0.5, ratio(), quiet, quiet, 0.0]);
        to_floor(&mut buf);
        assert_eq!(buf.frames(), 3, "ends on the last frame at the floor");
    }

    #[test]
    fn a_swell_after_a_dip_is_kept() {
        let mut buf = signal(&[1.0, 0.0, 0.0, 0.2, 0.0]);
        to_floor(&mut buf);
        assert_eq!(buf.frames(), 4);
    }

    #[test]
    fn the_floor_follows_the_peak() {
        let shape = [1.0, 0.1, 0.01, 0.002, 0.0005, 0.0];
        let lengths: Vec<usize> = [1.0, 0.05]
            .iter()
            .map(|gain| {
                let scaled: Vec<f32> = shape.iter().map(|s| s * gain).collect();
                let mut buf = signal(&scaled);
                to_floor(&mut buf);
                buf.frames()
            })
            .collect();
        assert_eq!(lengths, vec![4, 4]);
    }

    #[test]
    fn either_channel_holds_the_file_open() {
        let mut buf = Stereo::silence(5);
        buf.l[0] = 1.0;
        buf.r[3] = 0.5;
        to_floor(&mut buf);
        assert_eq!(buf.frames(), 4, "the right channel's late sound is kept");
    }

    #[test]
    fn silence_keeps_one_frame() {
        let mut buf = Stereo::silence(100);
        to_floor(&mut buf);
        assert_eq!(buf.frames(), 1);
    }
}
