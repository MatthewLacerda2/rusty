use std::borrow::Cow;
use std::path::{Path, PathBuf};

use super::{decode_bytes, wav};

#[derive(Debug, Default, PartialEq)]
pub struct Refresh {
    pub converted: Vec<(PathBuf, PathBuf)>,
    pub skipped: Vec<(PathBuf, String)>,
}

impl Refresh {
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

/// Convert every `.mp3` under `root` to a `.wav` beside it and remove the `.mp3`. One whose
/// `.wav` sibling exists is left alone (never overwrite), as is one that fails to decode (maybe
/// still being copied; the next refresh retries).
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

/// Atomic write, noted through `core::fs::written`: a reader never sees half a WAV and a clip
/// cache drops the old one.
fn convert(mp3: &Path, wav: &Path) -> Result<(), String> {
    let bytes = std::fs::read(mp3).map_err(|e| e.to_string())?;
    let audio = decode_bytes(bytes)?;
    let encoded = wav::encode(audio.channels, audio.sample_rate, &audio.samples);
    rusty_core::fs::written::write(wav, &encoded).map_err(|e| e.to_string())?;
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
