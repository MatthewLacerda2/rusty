//! Whether a stale or missing cache may be converted (#1253).
//!
//! Converting is authoring work (CLAUDE.md, *One format inside*): a conversion is
//! slow, and a cache re-made mid-game would hand new instances of a model geometry
//! its earlier instances don't have. Unity's runtime loads never import either; it
//! refreshes the asset database before Play instead. So `app::GameWorld` brings
//! every converted source in the project up to date ([`refresh_sources`]) and then
//! takes a [`PlayHold`] for as long as it plays; the standalone player [`ship`]s,
//! for good, since a shipped game reads its caches and nothing else. Conversion
//! runs only while no hold is taken and nothing shipped.
//!
//! While it is off, [`cached_gltf`](super::cached_gltf) serves the cache as it
//! stands, current or not, and holds back each source it found changed;
//! [`convert_held`] converts them once conversion is on again (the first edit
//! frame after Stop), and the swap's note reloads every entity using the model
//! (#1242). A source with no cache at all fails its load.
//!
//! Process-wide, as the cache and its lock are (per thread under test). The play
//! state lives in `app`, above this module, so `app` holds the switch and this
//! module never reads it.

use super::{cached_gltf, converter_for, is_import_cache, stale, Converter};
use crate::asset::ImportError;
use std::path::{Path, PathBuf};

/// Who refuses conversion, and the sources found changed while it was refused.
struct Gate {
    /// Live [`PlayHold`]s: one per world in Play.
    plays: usize,
    /// A shipped game: never converts, whatever plays.
    shipped: bool,
    /// Sources found changed while refused, in the order found.
    held: Vec<PathBuf>,
    /// Sources with no cache already warned of while refused.
    missing: Vec<PathBuf>,
}

impl Gate {
    fn allowed(&self) -> bool {
        self.plays == 0 && !self.shipped
    }
}

/// On until a world plays or the game ships.
const ON: Gate = Gate {
    plays: 0,
    shipped: false,
    held: Vec::new(),
    missing: Vec::new(),
};

#[cfg(not(test))]
static GATE: std::sync::Mutex<Gate> = std::sync::Mutex::new(ON);

/// Run `f` on the gate.
#[cfg(not(test))]
fn with_gate<T>(f: impl FnOnce(&mut Gate) -> T) -> T {
    // Nothing a panic could leave half-written: counters and lists of paths.
    f(&mut GATE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
}

// Per thread under test: `cargo test` (and cargo-mutants) runs every test of the
// binary in one process, and one test's Play must not stop another's import.
#[cfg(test)]
thread_local! {
    static GATE: std::cell::RefCell<Gate> = const { std::cell::RefCell::new(ON) };
}

#[cfg(test)]
fn with_gate<T>(f: impl FnOnce(&mut Gate) -> T) -> T {
    GATE.with(|gate| f(&mut gate.borrow_mut()))
}

/// Conversion refused while this lives: a world takes one as it enters Play and
/// drops it at Stop, or when the world itself is dropped, so a world that never
/// stops cannot leave conversion off for the rest of the process.
#[must_use = "conversion is refused only while the hold lives"]
pub struct PlayHold(());

impl PlayHold {
    /// Refuse conversion until the hold drops.
    pub fn take() -> Self {
        with_gate(|gate| gate.plays += 1);
        Self(())
    }
}

impl Drop for PlayHold {
    fn drop(&mut self) {
        with_gate(|gate| gate.plays -= 1);
    }
}

/// Refuse conversion for the rest of the process: a shipped game (the standalone
/// player) reads the caches it ships with.
pub fn ship() {
    with_gate(|gate| gate.shipped = true);
}

/// Whether a stale or missing cache is converted now.
pub fn conversion_allowed() -> bool {
    with_gate(|gate| gate.allowed())
}

/// Bring every converted source under `root` up to date, as Unity refreshes its
/// asset database before Play: a model only a script spawns has a cache to play
/// from. A source whose conversion fails is warned of and skipped. Returns how
/// many sources were looked at; none while conversion is refused.
pub fn refresh_sources(root: &Path) -> usize {
    if !conversion_allowed() {
        return 0;
    }
    let mut sources = Vec::new();
    collect_sources(root, &mut sources);
    sources.sort();
    for source in &sources {
        if let Err(e) = cached_gltf(source) {
            log::warn!("[Asset] {}: {e}", source.display());
        }
    }
    sources.len()
}

/// Every file under `dir` a converter reads, the import cache left out.
fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.flatten().map(|entry| entry.path()) {
        if is_import_cache(&path) {
            continue;
        } else if path.is_dir() {
            collect_sources(&path, out);
        } else if converter_for(&path).is_some() {
            out.push(path);
        }
    }
}

/// Convert every source held back while conversion was off, once it is on again.
/// Returns how many were converted; a failure is warned of, as an import's is.
/// A no-op while conversion is off, and cheap when nothing is held.
pub fn convert_held() -> usize {
    let held = with_gate(|gate| {
        if !gate.allowed() {
            return Vec::new();
        }
        gate.missing.clear();
        std::mem::take(&mut gate.held)
    });
    let mut converted = 0;
    for source in held {
        match cached_gltf(&source) {
            Ok(_) => converted += 1,
            Err(e) => log::warn!("[Asset] {}: {e}", source.display()),
        }
    }
    converted
}

/// `cached_gltf` with conversion off: the cache at `out` as it stands. A source
/// changed since is held back for [`convert_held`], and said so once; a source
/// never converted fails, since nothing may convert it now, and one that does not
/// exist fails as a missing file.
pub(super) fn serve(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    if !out.is_file() {
        return Err(not_imported(source));
    }
    let (held, shipped) = with_gate(|gate| (gate.held.iter().any(|p| p == source), gate.shipped));
    if held || stale(converter, source, out)?.is_none() {
        return Ok(());
    }
    with_gate(|gate| gate.held.push(source.to_path_buf()));
    let uses = if shipped {
        "the game uses the last import"
    } else {
        "Play uses the last import until Stop"
    };
    log::warn!(
        "[Asset] {}: changed since its import; {uses}",
        source.display()
    );
    Ok(())
}

/// Why a source with no cache fails while conversion is off, warned of once per
/// path: a missing file, or one never imported.
fn not_imported(source: &Path) -> ImportError {
    if !source.is_file() {
        return ImportError::Parse(format!("{}: file not found", source.display()));
    }
    let first = with_gate(|gate| {
        let first = !gate.missing.iter().any(|p| p == source);
        if first {
            gate.missing.push(source.to_path_buf());
        }
        first
    });
    if first {
        log::warn!(
            "[Asset] {}: not imported yet, and Play does not import; import it in edit mode first",
            source.display()
        );
    }
    ImportError::Parse(format!(
        "{}: no imported copy to play from; import it in edit mode first",
        source.display()
    ))
}
