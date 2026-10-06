//! A line of speech as a brief: what is asked for, and the four reasons a line is
//! not ready to be asked for yet (#386).
//!
//! Every refusal here happens before a key is resolved or a cent estimated, and each
//! names the field to fix. None of them is an error in the batch sense: a line with
//! no voice yet is a line still being written ([`Incomplete`]).

use std::fmt;

use scorsese_core::{SpeechModel, MAX_CHARACTERS};
use serde::Serialize;

use super::super::generated::Brief;

/// The model a brief that names none speaks with: Flash. `multilingual_v2` and `v3`
/// cost the same as each other, so the only real choice is *fast or not*, and a
/// default nobody set should not quietly be the dearer one.
pub const DEFAULT_MODEL: SpeechModel = SpeechModel::Fast;

/// One line, as the vendor will be asked for it. Its serialised fields are the
/// request, so they are what [`super::super::generated::digest`] hashes: change a
/// word, the voice, the model, the language or the seed and it is a new file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechBrief {
    /// The words.
    pub text: String,
    /// The ElevenLabs voice id. No default exists: every vendor default voice
    /// expires on 2026-12-31, so a voice written here would be an outage with a date.
    pub voice: String,
    /// Which model reads it, serialised as the vendor's id (`eleven_flash_v2_5`,
    /// `eleven_multilingual_v2`, `eleven_v3`). Always present, so naming the default
    /// and omitting it hash alike.
    #[serde(serialize_with = "vendor_id")]
    pub model: SpeechModel,
    /// An ISO 639-1 code pinning the reading's language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Best-effort at the vendor: the same seed usually reads the same way, and is
    /// not promised to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u32>,
}

impl Brief for SpeechBrief {
    fn kind(&self) -> &'static str {
        "speech"
    }

    /// WAV, not the MP3 the vendor sends: the bytes are converted on arrival (#385).
    fn extension(&self) -> &'static str {
        "wav"
    }
}

/// A brief as an author wrote it: any field may still be missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Draft {
    pub text: Option<String>,
    pub voice: Option<String>,
    pub model: Option<String>,
    pub language: Option<String>,
    pub seed: Option<u32>,
}

/// Why a line cannot be spoken yet. Each is something its author can finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incomplete {
    /// No words, or only whitespace.
    NoText,
    /// More characters than one request speaks.
    TooLong { found: usize },
    /// Nobody chosen to say it.
    NoVoice,
    /// A model id that is not one of the three.
    UnknownModel(String),
    /// `language` on `eleven_multilingual_v2`, which accepts it and ignores it.
    LanguageIgnored { language: String },
}

impl fmt::Display for Incomplete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoText => write!(f, "`text` is empty: there is nothing to speak"),
            Self::TooLong { found } => write!(
                f,
                "`text` is {found} characters; one request speaks at most {MAX_CHARACTERS}"
            ),
            Self::NoVoice => write!(
                f,
                "`voice` is missing: choose an ElevenLabs voice id (there is no default)"
            ),
            Self::UnknownModel(model) => write!(
                f,
                "`model` {model:?} is not one of {}",
                model_ids().join(", ")
            ),
            Self::LanguageIgnored { language } => write!(
                f,
                "`language` {language:?} is ignored by {}: drop it or pick another model",
                SpeechModel::Standard.model_id()
            ),
        }
    }
}

impl Draft {
    /// The brief this draft asks for, or the first thing it is missing.
    pub fn gather(self) -> Result<SpeechBrief, Incomplete> {
        let text = self.text.filter(|t| !t.trim().is_empty());
        let text = text.ok_or(Incomplete::NoText)?;
        let found = text.chars().count();
        if found > MAX_CHARACTERS {
            return Err(Incomplete::TooLong { found });
        }
        let voice = self.voice.filter(|v| !v.trim().is_empty());
        let voice = voice.ok_or(Incomplete::NoVoice)?;
        let model = match self.model {
            None => DEFAULT_MODEL,
            Some(id) => model_of(&id).ok_or(Incomplete::UnknownModel(id))?,
        };
        if let Some(language) = self.language.as_ref() {
            if !model.takes_language() {
                let language = language.clone();
                return Err(Incomplete::LanguageIgnored { language });
            }
        }
        Ok(SpeechBrief {
            text,
            voice,
            model,
            language: self.language,
            seed: self.seed,
        })
    }
}

fn vendor_id<S: serde::Serializer>(model: &SpeechModel, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(model.model_id())
}

/// The model a vendor id names.
fn model_of(id: &str) -> Option<SpeechModel> {
    SpeechModel::ALL.into_iter().find(|m| m.model_id() == id)
}

fn model_ids() -> Vec<&'static str> {
    SpeechModel::ALL.iter().map(|m| m.model_id()).collect()
}
