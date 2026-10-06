//! `Speech`: a line of text becomes a WAV the engine plays (#386).
//!
//! The adapter half. The vendor's half is scorsese's (`scorsese_providers::api::
//! elevenlabs`): the one POST, the `xi-api-key` header, the MP3 coming back, the
//! sorting of a refusal into advice, and the dated rate table. What lives here is
//! rusty's side of the line: the brief ([`SpeechBrief`]) and its local refusals, the
//! [`Speaker`] trait every test mocks, and [`generate`], which runs one line through
//! the boundary in the order every generating verb follows.
//!
//! **One call, not three.** Speech comes back on the same connection, so there is no
//! ticket, no polling and no state for work in flight.
//!
//! **The one non-deterministic asset in the engine.** A `seed` is sent and is
//! best-effort at the vendor: the same brief usually reads the same way, and is not
//! promised to. The cache is what makes a line reproducible: asked again, the brief
//! finds its file and nothing is called.

mod brief;

use std::fmt;

use scorsese_providers::api::elevenlabs::refusal::Refusal as VendorRefusal;
use scorsese_providers::api::elevenlabs::speech::{Speak, Speech};
use scorsese_providers::api::http::HttpError;

pub use brief::{Draft, Incomplete, SpeechBrief, DEFAULT_MODEL};

use super::generated::{self, Realised};
pub use super::Context;
use super::{permit, prices, Provider, Refusal, Secret};

/// Somewhere a line can be spoken. **The call that spends**, and the seam every
/// test replaces: no test ever holds the real one.
pub trait Speaker {
    /// Speak `body` in `voice`, authenticating with `key`; the vendor's MP3 bytes, or
    /// the vendor's own sentence about why not.
    fn speak(&self, key: &Secret, voice: &str, body: &Speak) -> Result<Vec<u8>, String>;
}

/// ElevenLabs, through scorsese's client.
#[derive(Debug, Clone, Copy)]
pub struct ElevenLabs;

impl Speaker for ElevenLabs {
    fn speak(&self, key: &Secret, voice: &str, body: &Speak) -> Result<Vec<u8>, String> {
        Speech::new(key)
            .speak(voice, body)
            .map_err(|error| match error {
                // The vendor's body, sorted into advice (a bad key, a missing scope,
                // a voice withdrawn), never just a status code.
                HttpError::Refused { status, body, .. } => {
                    VendorRefusal::of(status, voice, &body).to_string()
                }
                other => other.to_string(),
            })
    }
}

/// The vendor's request body for `brief`. The voice is not in it: the vendor puts
/// it in the URL.
pub fn request(brief: &SpeechBrief) -> Speak {
    Speak {
        text: brief.text.clone(),
        model_id: brief.model.model_id().to_owned(),
        language_code: brief.language.clone(),
        seed: brief.seed,
    }
}

/// What speaking `brief` is estimated to cost, in US cents: its characters at the
/// model's dated rate, rounded up so a run can never creep past the budget.
pub fn estimate_cents(brief: &SpeechBrief) -> Result<u64, SpeechError> {
    prices::speech(brief.model, brief.text.chars().count())
        .map(|estimate| estimate.cents)
        .map_err(|unpriced| SpeechError::Vendor(unpriced.to_string()))
}

/// Why a line did not come back as a file.
#[derive(Debug)]
pub enum SpeechError {
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

impl From<std::io::Error> for SpeechError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<Refusal> for SpeechError {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl fmt::Display for SpeechError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete(why) => write!(f, "speech brief incomplete: {why}"),
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::Vendor(why) => write!(f, "{why}"),
            Self::Unplayable(why) => write!(f, "the vendor's audio did not decode: {why}"),
            Self::Io(e) => write!(f, "writing the generated speech: {e}"),
        }
    }
}

impl std::error::Error for SpeechError {}

/// Speak one line, or find it already spoken.
///
/// Edit mode first (the verb is authoring, cache hit or not), then the brief's local
/// refusals, then the cache; only a miss estimates, takes a [`permit`], calls
/// `speaker` and records the spend, and the MP3 is converted to WAV before it is
/// stored, so what lands is a file the engine plays.
pub fn generate(
    draft: Draft,
    at: &Context<'_>,
    speaker: &dyn Speaker,
) -> Result<Realised, SpeechError> {
    if at.playing {
        return Err(Refusal::Playing.into());
    }
    let brief = draft.gather().map_err(SpeechError::Incomplete)?;
    generated::realise(at.root, &brief, at.today, |brief| {
        let estimate = estimate_cents(brief)?;
        let permit = permit(
            at.playing,
            Provider::ElevenLabs,
            estimate,
            at.environment,
            at.ledger,
        )?;
        let spoken = speaker.speak(permit.key(), &brief.voice, &request(brief));
        let mp3 = spoken.map_err(SpeechError::Vendor)?;
        // Recorded the moment the vendor answers: from here on the money is spent,
        // whatever becomes of the bytes.
        permit.record(at.ledger)?;
        let wav = generated::to_wav(mp3).map_err(SpeechError::Unplayable)?;
        Ok((wav, estimate))
    })
}
