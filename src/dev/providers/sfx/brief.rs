//! A sound effect as a brief: what is asked for, and why a brief is not ready to be
//! asked for yet (#388).
//!
//! Every refusal here happens before a key is resolved or a cent estimated, and each
//! names the field to fix. The bounds are the endpoint's own (checked 2026-10-06,
//! see #388), so an out-of-range length fails here naming itself rather than as a
//! remote 422 after the request went out.

use std::fmt;

use serde::Serialize;

use super::super::generated::Brief;

/// The shortest effect the endpoint makes, in seconds.
pub const MIN_SECONDS: f64 = 0.5;
/// The longest effect the endpoint makes, in seconds.
pub const MAX_SECONDS: f64 = 30.0;
/// How closely the vendor follows the words when a brief does not say: its default.
pub const DEFAULT_PROMPT_INFLUENCE: f64 = 0.3;
/// The one sound-effects model the vendor lists. Pinned in every brief, so a model
/// the vendor adds later can never quietly change what a cached brief meant.
pub const MODEL: &str = "eleven_text_to_sound_v2";

/// One effect, as the vendor will be asked for it. Its serialised fields are the
/// request, so they are what [`super::super::generated::digest`] hashes: change a
/// word, the length, the influence or the loop flag and it is a new file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SfxBrief {
    /// The sentence describing the sound.
    pub text: String,
    /// The length, in seconds. Required: a one-shot that has to fit an animation has
    /// a duration, and the vendor's own guess is neither reproducible nor priced
    /// with any certainty (#388).
    pub seconds: f64,
    /// 0 to 1: higher follows the words more closely and varies less. Always
    /// present, so naming the default and omitting it hash alike.
    pub prompt_influence: f64,
    /// Whether the effect loops seamlessly (ambience, a running engine).
    #[serde(rename = "loop", skip_serializing_if = "is_false")]
    pub looping: bool,
    /// The vendor's model id ([`MODEL`]).
    pub model: &'static str,
}

impl Brief for SfxBrief {
    fn kind(&self) -> &'static str {
        "sfx"
    }

    /// WAV, not the MP3 the vendor sends: the bytes are converted on arrival (#385).
    fn extension(&self) -> &'static str {
        "wav"
    }
}

/// A brief as an author wrote it: any field may still be missing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Draft {
    pub text: Option<String>,
    pub seconds: Option<f64>,
    pub prompt_influence: Option<f64>,
    pub looping: Option<bool>,
}

/// Why an effect cannot be generated yet. Each is something its author can finish.
#[derive(Debug, Clone, PartialEq)]
pub enum Incomplete {
    /// No words, or only whitespace.
    NoText,
    /// No length given.
    NoSeconds,
    /// A length the endpoint does not make.
    SecondsOutOfRange(f64),
    /// A prompt influence outside 0 to 1.
    InfluenceOutOfRange(f64),
}

impl fmt::Display for Incomplete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoText => write!(f, "`text` is empty: describe the sound"),
            Self::NoSeconds => write!(
                f,
                "`seconds` is missing: give the length, {MIN_SECONDS} to {MAX_SECONDS}"
            ),
            Self::SecondsOutOfRange(s) => write!(
                f,
                "`seconds` is {s}; the endpoint makes {MIN_SECONDS} to {MAX_SECONDS}"
            ),
            Self::InfluenceOutOfRange(p) => {
                write!(f, "`prompt_influence` is {p}; it must be 0 to 1")
            }
        }
    }
}

impl Draft {
    /// The brief this draft asks for, or the first thing it is missing.
    pub fn gather(self) -> Result<SfxBrief, Incomplete> {
        let text = self.text.filter(|t| !t.trim().is_empty());
        let text = text.ok_or(Incomplete::NoText)?;
        let seconds = self.seconds.ok_or(Incomplete::NoSeconds)?;
        // `contains` is false for NaN, so a NaN is refused with the rest.
        if !(MIN_SECONDS..=MAX_SECONDS).contains(&seconds) {
            return Err(Incomplete::SecondsOutOfRange(seconds));
        }
        let prompt_influence = self.prompt_influence.unwrap_or(DEFAULT_PROMPT_INFLUENCE);
        if !(0.0..=1.0).contains(&prompt_influence) {
            return Err(Incomplete::InfluenceOutOfRange(prompt_influence));
        }
        Ok(SfxBrief {
            text,
            seconds,
            prompt_influence,
            looping: self.looping.unwrap_or(false),
            model: MODEL,
        })
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
fn is_false(b: &bool) -> bool {
    !*b
}
