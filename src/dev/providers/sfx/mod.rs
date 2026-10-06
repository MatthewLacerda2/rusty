//! `Sfx`: a sentence becomes a one-shot sound effect the engine plays (#388).
//!
//! The recorded-world half of a shooter's sound: doors, glass, gravel, cloth,
//! mechanisms, the textures `Sound.*`'s synthesis cannot reach. The recipe stays the
//! first thing to reach for (free, offline, byte-reproducible); this is the paid
//! path, in its own namespace so nobody spends by accident.
//!
//! **Same lifecycle as speech, no exceptions**: [`generate`] runs Play check →
//! brief → cache → [`permit`] → the one call → record → MP3→WAV → store.
//!
//! **The endpoint is declared here**, because the pinned scorsese-providers has no
//! sound-effects client. Only the declaration is rusty's: the wire is scorsese's
//! [`Caller`], and a refusal is sorted by scorsese's [`VendorRefusal::of`], so no
//! HTTP code lives in this tree. The endpoint takes no seed; the cache is what makes
//! an effect reproducible.

mod brief;
pub mod price;

use std::fmt;

use scorsese_providers::api::elevenlabs::refusal::Refusal as VendorRefusal;
use scorsese_providers::api::http::{Caller, HttpError};
use serde::Serialize;

pub use brief::{Draft, Incomplete, SfxBrief, MAX_SECONDS, MIN_SECONDS, MODEL};
pub use price::estimate_cents;

use super::generated::{self, Realised};
use super::{permit, Context, Provider, Refusal, Secret};

/// The endpoint, asking for the one MP3 format every tier gets (192 kbps needs
/// Creator, PCM needs Pro).
const URL: &str = "https://api.elevenlabs.io/v1/sound-generation?output_format=mp3_44100_128";

/// The header ElevenLabs reads the key from.
const KEY_HEADER: &str = "xi-api-key";

/// The most an effect is read to, in bytes: 30 s of 128 kbps MP3 is under 0.5 MB,
/// so this bounds a reply gone wrong, not any real effect.
const MAX_AUDIO_BYTES: u64 = 16 * 1024 * 1024;

/// The whole POST body, field for field as the vendor names them.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Generate {
    pub text: String,
    pub duration_seconds: f64,
    pub prompt_influence: f64,
    #[serde(rename = "loop")]
    pub looping: bool,
    pub model_id: String,
}

/// Somewhere an effect can be made. **The call that spends**, and the seam every
/// test replaces: no test ever holds the real one.
pub trait Foley {
    /// Make `body`, authenticating with `key`; the vendor's MP3 bytes, or the
    /// vendor's own sentence about why not.
    fn make(&self, key: &Secret, body: &Generate) -> Result<Vec<u8>, String>;
}

/// ElevenLabs' sound-generation endpoint, through scorsese's transport.
#[derive(Debug, Clone, Copy)]
pub struct ElevenLabs;

impl Foley for ElevenLabs {
    fn make(&self, key: &Secret, body: &Generate) -> Result<Vec<u8>, String> {
        Caller::new(KEY_HEADER, key)
            .post_bytes(URL, body, MAX_AUDIO_BYTES)
            .map_err(|error| match error {
                HttpError::Refused { status, body, .. } => refusal(status, &body),
                other => other.to_string(),
            })
    }
}

/// The vendor's refusal as advice. scorsese's sorter reads a 404 as a withdrawn
/// voice, which this endpoint has none of, so that one keeps the vendor's words.
fn refusal(status: u16, body: &str) -> String {
    match VendorRefusal::of(status, "", body) {
        VendorRefusal::VoiceGone { .. } => format!("ElevenLabs answered {status}: {body}"),
        sorted => sorted.to_string(),
    }
}

/// The vendor's request body for `brief`.
pub fn request(brief: &SfxBrief) -> Generate {
    Generate {
        text: brief.text.clone(),
        duration_seconds: brief.seconds,
        prompt_influence: brief.prompt_influence,
        looping: brief.looping,
        model_id: brief.model.to_owned(),
    }
}

/// Why an effect did not come back as a file.
#[derive(Debug)]
pub enum SfxError {
    /// The brief is not finished. Nothing was resolved or spent, and a batch goes on.
    Incomplete(Incomplete),
    /// The boundary refused before spending: Play, no key, the budget.
    Refused(Refusal),
    /// The vendor took the call and refused it, in its own words.
    Vendor(String),
    /// The vendor's bytes would not decode to audio. The spend is already recorded.
    Unplayable(String),
    /// The file or its sidecar could not be written.
    Io(std::io::Error),
}

impl From<std::io::Error> for SfxError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Refusal> for SfxError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl fmt::Display for SfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete(why) => write!(f, "sfx brief incomplete: {why}"),
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::Vendor(why) => write!(f, "{why}"),
            Self::Unplayable(why) => write!(f, "the vendor's audio did not decode: {why}"),
            Self::Io(e) => write!(f, "writing the generated effect: {e}"),
        }
    }
}

impl std::error::Error for SfxError {}

/// Make one effect, or find it already made: speech's order exactly.
pub fn generate(draft: Draft, at: &Context<'_>, foley: &dyn Foley) -> Result<Realised, SfxError> {
    if at.playing {
        return Err(Refusal::Playing.into());
    }
    let brief = draft.gather().map_err(SfxError::Incomplete)?;
    generated::realise(at.root, &brief, at.today, |brief| {
        let estimate = estimate_cents(brief.seconds);
        let permit = permit(
            at.playing,
            Provider::ElevenLabs,
            estimate,
            at.environment,
            at.ledger,
        )?;
        let mp3 = foley.make(permit.key(), &request(brief));
        let mp3 = mp3.map_err(SfxError::Vendor)?;
        // Recorded the moment the vendor answers: from here on the money is spent.
        permit.record(at.ledger)?;
        let wav = generated::to_wav(mp3).map_err(SfxError::Unplayable)?;
        Ok((wav, estimate))
    })
}
