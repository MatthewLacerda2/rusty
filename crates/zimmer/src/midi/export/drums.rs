//! Which tracks are drums: the one fact an export needs that a song does not
//! hold.
//!
//! A song has no drum tracks, only tracks — a kick is a sine with a falling
//! pitch envelope, played at whatever note its author tuned it to — so
//! whether one belongs on General MIDI's percussion channel is asked, never
//! inferred. Guessing from the patch (noise is a drum, a pitch sweep is a
//! kick) would put a riser on the drum kit the first time a sound designer
//! made one out of noise.

use std::fmt;
use std::str::FromStr;

use super::ExportError;
use crate::Song;

/// A song track to write on channel 10, the percussion channel.
///
/// Written `snare` to keep every note's key, or `kick=36` to play every note
/// of the track on that one key — which is how a kick written at `C2` lands
/// on General MIDI's kick (36) rather than on whatever drum its pitch
/// happens to name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drum {
    /// The song track, by the name its notes use.
    pub track: String,
    /// The key every note of it is played on, or `None` to keep each note's
    /// own — the right reading for a drum part that is already a kit, such as
    /// one [imported](crate::midi::import) from a file's channel 10.
    pub key: Option<u8>,
}

impl FromStr for Drum {
    type Err = String;

    /// Reads `track` or `track=key`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (track, key) = match text.rsplit_once('=') {
            Some((track, key)) => {
                let key = key.trim().parse::<u8>().ok().filter(|key| *key <= 127);
                let Some(key) = key else {
                    return Err(format!(
                        "`{text}`: the key after `=` is a MIDI key, 0 to 127 — 36 is a kick"
                    ));
                };
                (track, Some(key))
            }
            None => (text, None),
        };
        let track = track.trim();
        if track.is_empty() {
            return Err(format!(
                "`{text}` names no track — write `kick` or `kick=36`"
            ));
        }
        Ok(Self {
            track: track.to_owned(),
            key,
        })
    }
}

impl fmt::Display for Drum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.key {
            Some(key) => write!(f, "{}={key}", self.track),
            None => f.write_str(&self.track),
        }
    }
}

/// How one song track is written: on a pitched channel, or as drums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Voice {
    /// Its notes at their pitches, on a channel of its own.
    Pitched,
    /// On channel 10, at each note's own key or at the one given.
    Drum(Option<u8>),
}

/// One [`Voice`] per song track, in the song's order — or a refusal for a
/// drum that names no track or no key. Named twice, the later one wins.
pub(super) fn resolve(song: &Song, drums: &[Drum]) -> Result<Vec<Voice>, ExportError> {
    let mut voices = vec![Voice::Pitched; song.tracks.len()];
    for drum in drums {
        if let Some(key) = drum.key.filter(|key| *key > 127) {
            return Err(ExportError::DrumKey {
                track: drum.track.clone(),
                key,
            });
        }
        let index = song
            .tracks
            .iter()
            .position(|track| track.name == drum.track)
            .ok_or_else(|| ExportError::NoSuchTrack {
                track: drum.track.clone(),
            })?;
        voices[index] = Voice::Drum(drum.key);
    }
    Ok(voices)
}
