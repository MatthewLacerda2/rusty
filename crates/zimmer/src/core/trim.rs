//! trim — where a baked one-shot's file ends.
//!
//! [`super::render_note`] sizes its buffer for the **worst case**: the gate,
//! the whole release, and every effect's tail at its longest
//! ([`crate::fx::tail_seconds`]). That is right while rendering, because a
//! buffer cut short of a reverb's ring-out is a click. It is wrong as the
//! length of a file. A 0.1 s pop through a small room was baked to 1.2 s,
//! almost all of it far below anything heard (#970), and a clip placed for the
//! whole of its source blocks its track for as long as the file runs.
//!
//! So a one-shot's bake ends where its signal falls below [`FLOOR_DB`] **for
//! good**: the last frame on either channel that still reaches the floor is
//! the last frame kept. A sound that dips and swells again keeps the swell,
//! because the search runs back from the end rather than forward from the
//! start.
//!
//! ## Why trimming rather than a `tail` field
//!
//! A song has `tail` (`ring` / `exact` / `wrap`) because those are three
//! different musical answers to "how does this end", and a loop needs one of
//! them. A one-shot has one right answer, which is *when the sound does*. A
//! field whose only sensible value is its default is a knob nobody should
//! turn, and every patch already on disk gets the fix without being edited.
//!
//! ## Why relative to the peak
//!
//! The floor is measured down from the file's own loudest sample rather than
//! from full scale. A soft footstep at velocity 0.2 rings out exactly like a
//! hard one, only quieter, and an absolute floor would cut its room short
//! while leaving a loud one's alone. The mix decides how loud a clip plays;
//! the file should be the same shape either way.
//!
//! Only the bake is trimmed. A song renders its notes untrimmed and decides
//! its own length with `tail`, and [`crate::render_note`] hands a caller the
//! raw buffer, for the same reason it hands it unlimited.

use crate::level::db_to_gain_f32;
use crate::stereo::Stereo;

/// How far below its own peak a one-shot has to fall, in dB, for the rest of
/// its file to be dropped.
///
/// −60 dB is RT60, the decay acoustics has always used to say a room has
/// stopped ringing: a thousandth of the peak's amplitude. Below it a tail is
/// under the noise floor of any room the video will be played in, and under
/// any other sound on the timeline by a wide margin. The cut itself is a step
/// of at most that thousandth, which is inaudible without a fade.
pub(crate) const FLOOR_DB: f32 = -60.0;

/// Drops everything after the last frame that reaches [`FLOOR_DB`] below the
/// buffer's peak. A silent buffer keeps one frame, so a bake is never an
/// empty file.
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

    /// A mono signal, centred.
    fn signal(samples: &[f32]) -> Stereo {
        Stereo::centred(samples.to_vec())
    }

    /// The floor as an amplitude ratio, for building probes either side of it.
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

    /// The floor is the peak's, so a soft sound keeps the same shape of tail
    /// a loud one does.
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
