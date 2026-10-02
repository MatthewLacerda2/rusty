//! src/asset/audio/import.rs — MP3 arrives, WAV stays (#385).
//!
//! Two doors in: [`refresh`] sweeps a project tree for `.mp3` files (the editor runs
//! it at boot and whenever its window regains focus, Unity's auto-refresh; scripts
//! call it as `Assets.Refresh()`), and [`import_mp3_bytes`] takes a provider's
//! response body straight to a `.wav`. Both trim encoder delay and padding through
//! the shared decoder. A scene that still names `foo.mp3` keeps working:
//! [`resolve_clip_path`] sends it to the `foo.wav` that replaced it.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use super::{decode_bytes, wav};

/// What one [`refresh`] did: each MP3 converted (source → WAV written), and each it
/// left in place with the reason.
#[derive(Debug, Default, PartialEq)]
pub struct Refresh {
    pub converted: Vec<(PathBuf, PathBuf)>,
    pub skipped: Vec<(PathBuf, String)>,
}

impl Refresh {
    /// One console line per file: `(is_warning, text)`.
    pub fn lines(&self) -> Vec<(bool, String)> {
        let converted = self.converted.iter().map(|(mp3, wav)| {
            let text = format!("[Assets] {} -> {}", mp3.display(), wav.display());
            (false, text)
        });
        let skipped = self.skipped.iter().map(|(mp3, why)| {
            let text = format!("[Assets] {} left as MP3: {why}", mp3.display());
            (true, text)
        });
        converted.chain(skipped).collect()
    }
}

/// Convert every `.mp3` under `root` (recursively, in path order) to a `.wav` beside
/// it and remove the `.mp3`. An MP3 whose `.wav` sibling already exists is left
/// alone (never overwrite a file somebody made), as is one that fails to decode — a
/// file still being copied in, say, which the next refresh retries. Both still play
/// through the runtime's MP3 fallback meanwhile.
pub fn refresh(root: &Path) -> Refresh {
    let mut mp3s = Vec::new();
    collect_mp3s(root, &mut mp3s);
    mp3s.sort();
    let mut report = Refresh::default();
    for mp3 in mp3s {
        let wav = mp3.with_extension("wav");
        let result = if wav.exists() {
            Err(format!("{} already exists", wav.display()))
        } else {
            convert(&mp3, &wav)
        };
        match result {
            Ok(()) => report.converted.push((mp3, wav)),
            Err(why) => report.skipped.push((mp3, why)),
        }
    }
    report
}

/// Decode encoded audio `bytes` (a provider's MP3) and write it to `dest` as WAV,
/// replacing whatever is there. The write goes through a temporary file and a
/// rename, so a reader never sees half a WAV.
pub fn import_mp3_bytes(bytes: Vec<u8>, dest: &Path) -> Result<(), String> {
    let audio = decode_bytes(bytes)?;
    let encoded = wav::encode(audio.channels, audio.sample_rate, &audio.samples);
    let partial = dest.with_extension("wav.part");
    std::fs::write(&partial, encoded).map_err(|e| e.to_string())?;
    std::fs::rename(&partial, dest).map_err(|e| e.to_string())
}

/// Where a clip reference actually plays from: `foo.mp3` that has been converted
/// (gone, with `foo.wav` beside it) resolves to `foo.wav`; anything else is itself.
pub fn resolve_clip_path(path: &str) -> Cow<'_, str> {
    let source = Path::new(path);
    if !is_mp3(source) || source.exists() {
        return Cow::Borrowed(path);
    }
    let wav = source.with_extension("wav");
    match wav.exists() {
        true => Cow::Owned(wav.to_string_lossy().into_owned()),
        false => Cow::Borrowed(path),
    }
}

/// One MP3 file → its WAV, then the MP3 is removed.
fn convert(mp3: &Path, wav: &Path) -> Result<(), String> {
    let bytes = std::fs::read(mp3).map_err(|e| e.to_string())?;
    import_mp3_bytes(bytes, wav)?;
    std::fs::remove_file(mp3).map_err(|e| e.to_string())
}

fn collect_mp3s(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            collect_mp3s(&path, out);
        } else if is_mp3(&path) {
            out.push(path);
        }
    }
}

fn is_mp3(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
}
