//! Every voice on the account, every page of it: the one listing scorsese's catalogue
//! does not make.
//!
//! scorsese lists the *premade* set (`/v1/voices?category=premade`) and the Voice
//! Library; rusty wants what the account can actually speak with, cloned and
//! designed voices included. That is `/v2/voices`, which pages: up to
//! [`PAGE_SIZE`] voices a reply, `has_more` and a `next_page_token` for the rest.
//! Reading only the first page is the bug scorsese shipped and fixed (scorsese#240),
//! so [`every_page`] follows the token to the end, and a reply that says there is
//! more without saying where is an error rather than a short list.
//!
//! The lookup of one voice and the Voice Library are scorsese's calls unchanged
//! ([`ElevenLabsVoices`]), so a withdrawn voice is sorted by the same reading of
//! the vendor's 404 that `Speech.Generate` uses.

use serde::Deserialize;

use scorsese_providers::api::elevenlabs::voices::{Filters, Voice as Vendor};
use scorsese_providers::api::http::{Caller, HttpError};
use scorsese_providers::credentials::CredentialError;
use scorsese_providers::video::ProviderError;
use scorsese_providers::voices::{
    Availability, Catalogue, ElevenLabsVoices, Page, Voice, VoiceError,
};

use super::super::{resolve, Environment, Provider, Refusal, Secret};

/// The most voices `/v2/voices` puts on one page.
pub const PAGE_SIZE: u32 = 100;

/// More pages than any account has (100,000 voices): past it, the vendor is
/// repeating itself and the loop stops rather than spinning.
const MAX_PAGES: usize = 1000;

const NAME: &str = "ElevenLabs";

/// One page of `/v2/voices`. Read permissively: only what is acted on is named.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AccountPage {
    #[serde(default)]
    pub voices: Vec<Vendor>,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// Every page, in order, asking `fetch` for each one by the previous page's token.
pub fn every_page(
    mut fetch: impl FnMut(Option<&str>) -> Result<AccountPage, HttpError>,
) -> Result<Vec<Voice>, VoiceError> {
    let mut voices = Vec::new();
    let mut token: Option<String> = None;
    for _ in 0..MAX_PAGES {
        let page = fetch(token.as_deref())
            .map_err(|e| VoiceError::Provider(ProviderError::new(NAME, e)))?;
        voices.extend(page.voices.iter().map(flatten));
        if !page.has_more {
            return Ok(voices);
        }
        token = Some(
            page.next_page_token
                .filter(|t| !t.is_empty())
                .ok_or_else(|| {
                    cut_short(format!(
                        "it said there were more voices past {} but gave no page token",
                        voices.len()
                    ))
                })?,
        );
    }
    Err(cut_short(format!(
        "it was still paging after {MAX_PAGES} pages"
    )))
}

/// A listing the vendor would not finish: never served as if it were the whole list.
fn cut_short(why: String) -> VoiceError {
    VoiceError::Provider(ProviderError::new(
        NAME,
        format!("the voice listing stopped short: {why}"),
    ))
}

/// A vendor voice as the catalogue carries one: id, name, and every trait word the
/// listing filled in, top-level fields first, then labels not already said.
pub fn flatten(found: &Vendor) -> Voice {
    let mut traits: Vec<String> = Vec::new();
    let top = [
        &found.language,
        &found.gender,
        &found.age,
        &found.accent,
        &found.descriptive,
        &found.use_case,
    ];
    for value in top
        .into_iter()
        .flatten()
        .map(|v| v.trim())
        .chain(found.labels.values().map(|v| v.trim()))
    {
        if !value.is_empty() && !traits.iter().any(|had| had.eq_ignore_ascii_case(value)) {
            traits.push(value.to_owned());
        }
    }
    Voice {
        id: found.voice_id.clone(),
        name: found.name.clone(),
        traits,
        description: found.description.clone(),
        preview: found.preview_url.clone(),
    }
}

/// ElevenLabs as rusty's catalogue. The key is resolved per call, so a warm cache
/// still answers on a machine with no key; a cold one says where it looked.
pub struct Account<'a> {
    pub environment: &'a Environment,
}

impl Account<'_> {
    fn key(&self) -> Result<Secret, VoiceError> {
        resolve(Provider::ElevenLabs, self.environment).map_err(|refusal| match refusal {
            Refusal::MissingKey {
                provider,
                variable,
                looked_in,
            } => VoiceError::Credential(CredentialError::Missing {
                provider,
                places: format!("{looked_in}. Set {variable} (see .env.example)"),
            }),
            other => VoiceError::Provider(ProviderError::new(NAME, other)),
        })
    }
}

impl Catalogue for Account<'_> {
    /// Every voice the account has, not only the premade set (see the module doc).
    fn builtin(&self) -> Result<Vec<Voice>, VoiceError> {
        let caller = Caller::new("xi-api-key", &self.key()?);
        every_page(|token| caller.get(&page_url(token)))
    }

    fn library(&self, filters: &Filters) -> Result<Page, VoiceError> {
        ElevenLabsVoices::new(&self.key()?).library(filters)
    }

    fn one(&self, voice_id: &str) -> Result<Availability, VoiceError> {
        ElevenLabsVoices::new(&self.key()?).one(voice_id)
    }

    fn name(&self) -> &'static str {
        NAME
    }
}

/// `/v2/voices`, one page, after `token` when there is one.
pub fn page_url(token: Option<&str>) -> String {
    let mut url = format!("https://api.elevenlabs.io/v2/voices?page_size={PAGE_SIZE}");
    if let Some(token) = token {
        url.push_str("&next_page_token=");
        for byte in token.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    url.push(char::from(byte))
                }
                _ => url.push_str(&format!("%{byte:02X}")),
            }
        }
    }
    url
}
