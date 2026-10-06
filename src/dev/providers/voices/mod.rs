//! `Speech.Voices` / `FindVoice` / `VoiceInfo`: where a voice id comes from (#387).
//!
//! **Free.** Listing voices generates nothing and spends no credits, so nothing here
//! takes a [`permit`](super::permit) or touches the ledger. It still needs the key,
//! and it is still authoring: refused during Play like every provider verb.
//!
//! **No voice id is ever shipped.** Every ElevenLabs default voice expires on
//! 2026-12-31, so the list is resolved at runtime, and a voice that is gone is
//! reported as gone, never swapped for another (that would bill a batch in a voice
//! nobody chose).
//!
//! The machinery is scorsese's `voices` module, used as-is: the cache under the
//! project's `cache/voices/` (re-read after [`REFRESH_AFTER_DAYS`] or on demand,
//! and every answer saying how old it is), the fallback to a stale list when the
//! vendor cannot be asked, and the sorting of a 404 into [`Unusable::Gone`]. rusty
//! adds the account-wide paginated listing ([`account`]), the name/label filter and
//! the Play check.

mod account;

use std::fmt;
use std::path::Path;

pub use account::{every_page, flatten, page_url, Account, AccountPage, PAGE_SIZE};
pub use scorsese_providers::voices::{
    Answer, Availability, Catalogue, Freshness, Unusable, Voice, VoiceError, REFRESH_AFTER_DAYS,
};

use super::Refusal;

/// How a script forces a re-read, for the provenance line every listing carries.
pub const REFRESH: &str = "Speech.Voices(true)";

/// Why a voice question went unanswered.
#[derive(Debug)]
pub enum VoicesError {
    /// Refused before asking: Play.
    Refused(Refusal),
    /// An empty `FindVoice` query: it would match everything, which is `Voices()`.
    EmptyQuery,
    /// No key with a cold cache, a vendor that would not answer, a key without
    /// `voices_read`: scorsese's own sentence, which names the fix.
    Catalogue(VoiceError),
}

impl fmt::Display for VoicesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::EmptyQuery => write!(f, "Speech.FindVoice needs a name or a label to look for"),
            Self::Catalogue(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for VoicesError {}

fn edit_mode(playing: bool) -> Result<(), VoicesError> {
    if playing {
        Err(VoicesError::Refused(Refusal::Playing))
    } else {
        Ok(())
    }
}

/// Every voice on the account, out of `root`'s cache when it is current, from the
/// vendor when it is cold, stale, or `refresh` asks.
pub fn list(
    root: &Path,
    playing: bool,
    catalogue: &dyn Catalogue,
    refresh: bool,
) -> Result<Answer, VoicesError> {
    edit_mode(playing)?;
    scorsese_providers::voices::builtin(root, catalogue, refresh).map_err(VoicesError::Catalogue)
}

/// The same listing, kept to the voices whose name or a label contains `query`
/// (ignoring case). A filter over ElevenLabs' own labels, not a taxonomy.
pub fn find(
    root: &Path,
    playing: bool,
    catalogue: &dyn Catalogue,
    query: &str,
    refresh: bool,
) -> Result<Answer, VoicesError> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Err(VoicesError::EmptyQuery);
    }
    let mut answer = list(root, playing, catalogue, refresh)?;
    answer.voices.retain(|voice| matches(voice, &query));
    Ok(answer)
}

/// Whether `voice`'s name or one of its traits contains `query` (already lowercase).
pub fn matches(voice: &Voice, query: &str) -> bool {
    std::iter::once(&voice.name)
        .chain(&voice.traits)
        .any(|word| word.to_lowercase().contains(query))
}

/// Whether `voice_id` can still be spoken with. **Never cached**: this is the
/// question a cached list would answer with exactly the stale fact it is asking
/// about.
pub fn info(
    playing: bool,
    catalogue: &dyn Catalogue,
    voice_id: &str,
) -> Result<Availability, VoicesError> {
    edit_mode(playing)?;
    catalogue.one(voice_id).map_err(VoicesError::Catalogue)
}

/// What a script is told about a voice it cannot use: named, and never replaced.
pub fn withdrawn(voice_id: &str, why: &Unusable) -> String {
    let state = match why {
        Unusable::Gone => "withdrawn",
        Unusable::NotOnThisPlan { .. } => "not on this plan",
    };
    format!(
        "voice {voice_id} is {state}: {why}. Nothing was substituted: pick another \
         from Speech.Voices()"
    )
}
