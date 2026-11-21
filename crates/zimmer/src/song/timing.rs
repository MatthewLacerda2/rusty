//! Making a song the length the picture needs.
//!
//! A song's natural length is whatever its notes add up to, plus however long
//! the last one takes to stop ringing. That is the wrong answer for video,
//! where the music has a hole to fill: forty-three seconds between two cuts.
//! And it is the wrong answer for a game too, which is easy to miss: a game
//! jumps back to the first sample the moment the file ends, so the ring-out
//! sits at the end of every pass and stops dead under the next downbeat. A
//! loop that is seamless has to carry its own tail round to the start —
//! [`Tail::Wrap`].
//!
//! Everything here is optional, and absent means the song is as long as it is.

use serde::{Deserialize, Serialize};

use super::Song;
use crate::error::SynthError;

/// How far a tempo may be moved to make a song fit, as a fraction either way.
///
/// A bed at 40% speed is not a bed, it is a mistake — so `stretch` refuses
/// past this rather than delivering something nobody would use. A quarter is
/// already a large musical change; the point of the bound is that the refusal
/// says what tempo it would have needed, which is enough to decide what to do
/// instead.
pub(crate) const MAX_STRETCH: f32 = 0.25;

/// A length the song must come out at.
///
/// Written one of two ways, and exactly one: as a number of `seconds`, or as
/// `to` — where the number comes from, for a caller that knows a length this
/// crate cannot. The second is how a video says "as long as the clip that
/// plays me": the edit decides that length and moves it, so a number copied
/// into the recipe by hand goes stale the first time the cut changes (#1000).
///
/// This crate never learns what a clip is. Whoever reads a `to` resolves it
/// into seconds with [`Fit::resolved`] before rendering, and a song still
/// carrying an unresolved `to` is refused by [`crate::Song::validate`] rather
/// than rendered at some length nobody chose.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fit {
    /// The target length in seconds — the hole in the cut. Absent when `to`
    /// says where the length comes from instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seconds: Option<f32>,
    /// Where the target length comes from, when it is not written down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<FitTo>,
    /// How to get there.
    #[serde(default)]
    pub mode: FitMode,
}

impl Fit {
    /// A fit to a written number of seconds — what every fit was before `to`.
    pub fn lasting(seconds: f32, mode: FitMode) -> Self {
        Self {
            seconds: Some(seconds),
            to: None,
            mode,
        }
    }

    /// This fit with its length decided: `seconds`, and no `to` left to
    /// resolve. The mode is kept, which is the whole of what a `to` fit was
    /// asking for besides the number.
    pub fn resolved(self, seconds: f32) -> Self {
        Self::lasting(seconds, self.mode)
    }
}

/// Where a [`Fit`]'s length comes from, when the recipe does not write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitTo {
    /// The length of the clip that plays this song, read from the project by
    /// whoever bakes it — so a cut that gets shorter takes its music with it.
    Clip,
}

/// What a song does when it is not already the length it has to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Repeat the arrangement until the target is reached, cutting
    /// mid-arrangement if that is where the time runs out.
    ///
    /// The default, because it is what a bed under dialogue wants: the seam is
    /// inaudible under speech, and the music keeps its own tempo.
    #[default]
    Loop,
    /// Move the tempo so a whole number of arrangement passes lands exactly on
    /// the target.
    ///
    /// Keeps the music intact and changes how fast it is played instead.
    /// Refuses rather than distorts past a quarter either way.
    Stretch,
    /// Play through once and pad with silence to the target.
    ///
    /// Honest, and the right answer for a sting: a sound that happens once and
    /// then is over does not want to be looped into a texture.
    Once,
}

/// Level moves applied to the finished piece.
///
/// In the recipe rather than left to clip keyframes because a fade that
/// belongs to the *music* — a piece that ends by resolving and receding — is a
/// property of the piece, and should survive being moved elsewhere in the
/// timeline or used in another project. Keyframes on the clip stay the right
/// tool for ducking *this* use of it under *this* voice-over.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    /// Seconds from silence to full level at the start.
    #[serde(default)]
    pub in_seconds: f32,
    /// Seconds from full level to silence at the end.
    #[serde(default)]
    pub out_seconds: f32,
}

impl Fade {
    /// True when neither end does anything, so the whole pass can be skipped.
    pub(crate) fn is_silent_about_everything(self) -> bool {
        self.in_seconds <= 0.0 && self.out_seconds <= 0.0
    }
}

/// What happens after the last beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tail {
    /// Let it ring: the buffer grows to fit the last note's release and any fx
    /// tail, so the piece ends by stopping rather than by being stopped.
    #[default]
    Ring,
    /// End exactly on the arrangement's last beat, with the tail faded into
    /// it. What a caller wants when the music has to butt against something.
    Exact,
    /// End exactly on the arrangement's last beat, with everything that rings
    /// past it **summed back onto the start** — what a game's music wants,
    /// because the file is going to be played round and round.
    ///
    /// It is what a band playing the piece in a circle would sound like: the
    /// reverb of the last bar rings over the first. So the loop point is
    /// seamless by construction rather than merely quiet, which neither other
    /// tail can say — `ring` leaves the tail at the end of the file, where it
    /// is cut dead when playback jumps back, and `exact` fades it away.
    ///
    /// Summed rather than crossfaded, because a crossfade turns the start of
    /// every pass down, a level change nobody asked for. The fold happens
    /// before the master limiter, which is what keeps the sum from clipping,
    /// and that limiter reads the file as the circle it is, so nothing it does
    /// puts a seam back where the fold took one away.
    ///
    /// Refused, when rendered, if the tail is longer than the loop: it would
    /// still be ringing the next time round, and cutting it short would be the
    /// fault it exists to fix. Refused alongside a `fade` (a fade on a loop is
    /// a dip every pass) and a `fit` other than `stretch`, which is the one
    /// mode that lands on a whole number of passes and so has a loop point.
    Wrap,
}

/// Refuses what cannot join `tail: wrap`, because it would put a seam back
/// into the loop the tail makes.
///
/// - **Any `fade`.** A fade-out on a loop is a dip at every pass. A fade-in is
///   the same dip on the other side of the seam, and worse: it would turn down
///   the very tail the wrap just carried round. A player that wants the music
///   to come in gently fades the *voice* it plays the loop on, once.
/// - **`fit` in `loop` or `once`.** A loop fit cuts mid-pass, so the file has
///   no one point where the music comes back round; a once fit pads with
///   silence, which the loop would then play every time. `stretch` lands on
///   a whole number of passes, so it has a loop point, and it is allowed.
pub(crate) fn check_wrap(fit: Option<Fit>, fade: Option<Fade>) -> Result<(), SynthError> {
    if fade.is_some_and(|fade| !fade.is_silent_about_everything()) {
        return Err(SynthError::WrapWith {
            field: "`fade`",
            why: "on a loop it is a dip in the level every time round — fade the music in \
                  or out where it is played instead",
        });
    }
    let (field, why) = match fit.map(|fit| fit.mode) {
        Some(FitMode::Loop) => (
            "`fit` in mode `loop` (the default)",
            "it cuts mid-pass, so the file has no single point where the music comes back \
             round — use mode `stretch`",
        ),
        Some(FitMode::Once) => (
            "`fit` in mode `once`",
            "it pads with silence, which the loop would play every time round — use mode \
             `stretch`",
        ),
        Some(FitMode::Stretch) | None => return Ok(()),
    };
    Err(SynthError::WrapWith { field, why })
}

/// The length and level fields, checked here rather than at render time so
/// a song that cannot be made to fit says so before anything is rendered.
pub(super) fn check(song: &Song) -> Result<(), SynthError> {
    if let Some(fade) = song.fade
        && !(fade.in_seconds.is_finite()
            && fade.in_seconds >= 0.0
            && fade.out_seconds.is_finite()
            && fade.out_seconds >= 0.0)
    {
        return Err(SynthError::BadFade {
            seconds: fade.in_seconds.max(fade.out_seconds),
        });
    }
    if song.tail() == Tail::Wrap {
        check_wrap(song.fit, song.fade)?;
    }
    let Some(fit) = song.fit else {
        return Ok(());
    };
    let seconds = match (fit.seconds, fit.to) {
        (Some(seconds), None) => seconds,
        (None, None) => {
            return Err(SynthError::FitLength {
                why: "needs a length: `seconds`, or `to: \"clip\"` for as long as the \
                      clip that plays the song",
            });
        }
        (Some(_), Some(_)) => {
            return Err(SynthError::FitLength {
                why: "takes `seconds` or `to`, not both — `to` says where the length \
                      comes from, so a number beside it would be ignored",
            });
        }
        (None, Some(_)) => {
            return Err(SynthError::FitLength {
                why: "is `to: \"clip\"`, and nothing has said how long that clip is — \
                      it is resolved by whatever bakes the song from a project, which \
                      reads the clip; rendering the recipe alone needs `seconds`",
            });
        }
    };
    if !(seconds.is_finite() && seconds > 0.0) {
        return Err(SynthError::BadFitSeconds { seconds });
    }
    // A stretch beyond the bound would deliver something nobody would use,
    // so it is refused with the tempo it would have needed — which is the
    // number a caller needs to decide what to do instead. A song with
    // anchors is stretched span by span, and `anchor::check` bounds each.
    if song.anchors.is_empty()
        && let Some(ratio) = super::shape::stretch_ratio(song, fit, seconds)
        && ratio.abs() > MAX_STRETCH
    {
        return Err(SynthError::StretchTooFar {
            bpm: song.bpm,
            needed: song.bpm * (1.0 + ratio),
            limit: MAX_STRETCH,
        });
    }
    Ok(())
}
