//! Whether a stale or missing cache may be converted (#1253).
//!
//! Converting is authoring work (CLAUDE.md, *One format inside*): a conversion is
//! slow, and a cache re-made mid-game would hand new instances of a model geometry
//! its earlier instances don't have. Unity's runtime loads never import either; it
//! refreshes the asset database before Play instead. So `app::GameWorld` brings
//! every converted source in the project up to date ([`refresh_sources`]) and then
//! takes a [`PlayHold`] for as long as it plays, the standalone player too: a
//! project run from its folder may hold sources, while an exported one
//! (`dev::export`, #1267) has none, so its refresh finds nothing to convert.
//! Conversion runs only while no hold is taken.
//!
//! While it is off, [`cached_gltf`](super::cached_gltf) serves the cache as it
//! stands, current or not, and holds back each source it found changed;
//! [`convert_held`] converts them once conversion is on again (the first edit
//! frame after Stop), and the swap's note reloads every entity using the model
//! (#1242). A source with no cache at all fails its load. A source whose
//! conversion failed at the refresh but has an older cache plays from that one,
//! warned of as changed.
//!
//! Process-wide, as the cache and its lock are (per thread under test). The play
//! state lives in `app`, above this module, so `app` holds the switch and this
//! module never reads it.

use super::{cache_location, cached_gltf, converter_for, record, stale, Converter, Tool};
use crate::asset::ImportError;
use crate::core::fs::written::Stale;
use std::path::{Component, Path, PathBuf};

/// Who refuses conversion, and the sources found changed while it was refused.
struct Gate {
    /// Live [`PlayHold`]s: one per world in Play.
    plays: usize,
    /// A standalone game ([`mark_standalone`]): what its warnings say, nothing more.
    standalone: bool,
    /// Sources found changed while refused, in the order found.
    held: Vec<PathBuf>,
    /// Sources with no cache already warned of while refused.
    missing: Vec<PathBuf>,
}

impl Gate {
    fn allowed(&self) -> bool {
        self.plays == 0
    }

    /// What a held source plays from, in this run's words: a standalone game has no
    /// Stop to wait for.
    fn uses(&self) -> &'static str {
        if self.standalone {
            "the game uses the last import"
        } else {
            "Play uses the last import until Stop"
        }
    }
}

/// On until a world plays.
const ON: Gate = Gate {
    plays: 0,
    standalone: false,
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

/// Word warnings for a standalone game (the player), which has no edit mode to
/// return to. Conversion is refused by its [`PlayHold`], as the editor's is.
pub fn mark_standalone() {
    with_gate(|gate| gate.standalone = true);
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
    let sources = convertible_sources(root);
    for source in &sources {
        if let Err(e) = cached_gltf(source) {
            log::warn!("[Asset] {}: {e}", source.display());
        }
    }
    sources.len()
}

/// Every source under `root` a converter reads, sorted: what a refresh brings up to
/// date and an export (`dev::export`, #1267) converts and leaves behind.
pub fn convertible_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    crate::asset::manifest::walk(root, &mut |path| {
        if converter_for(path).is_some() {
            sources.push(path.to_path_buf());
        }
    });
    sources.sort();
    sources
}

/// The watch's refresh (#1327): only the sources a change touched. Play's refresh
/// checks every source; the editor's file watch must not, since a source whose
/// conversion fails (a `.blend` timing out in Blender) would run again on every frame
/// that noted anything. A failed source is left alone until it, or a file it read,
/// changes again.
/// Convert every source under `root` that `changed` names, or whose cached
/// conversion read a file `changed` names. Returns how many it converted; a failure
/// is warned of, as an import's is. A no-op while conversion is off.
pub fn refresh_changed(root: &Path, changed: Vec<String>) -> usize {
    if !conversion_allowed() {
        return 0;
    }
    let touched = touched(root, &Stale::new(changed));
    let mut converted = 0;
    for source in touched {
        match cached_gltf(&source) {
            Ok(_) => converted += 1,
            Err(e) => log::warn!("[Asset] {}: {e}", source.display()),
        }
    }
    converted
}

/// The sources under `root` a note in `changed` touches, sorted.
pub(super) fn touched(root: &Path, changed: &Stale) -> Vec<PathBuf> {
    if changed.is_empty() {
        return Vec::new();
    }
    let names = |path: &Path| changed.matches(&path.to_string_lossy());
    let read_one = |source: &Path| {
        let stored = cache_location(source)
            .ok()
            .and_then(|out| record::stored(&out));
        stored.is_some_and(|s| s.depends_on.iter().any(|d| names(d.path())))
    };
    convertible_sources(root)
        .into_iter()
        .filter(|source| names(source) || read_one(source))
        .collect()
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
/// exist fails as a missing file. A program a converter runs is never started
/// here, not even to ask its version: a cache of the unchanged source, made by
/// any version of it, is served (#1072).
pub(super) fn serve(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    if !out.is_file() {
        return Err(not_imported(source));
    }
    let key = key(source);
    if with_gate(|gate| gate.held.contains(&key))
        || stale(converter, source, out, Tool::unasked(converter))?.is_none()
    {
        return Ok(());
    }
    let uses = with_gate(|gate| {
        gate.held.push(key);
        gate.uses()
    });
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
    let key = key(source);
    let (first, standalone) = with_gate(|gate| {
        let first = !gate.missing.contains(&key);
        if first {
            gate.missing.push(key);
        }
        (first, gate.standalone)
    });
    let fix = if standalone {
        "a running game does not import; open the project in the editor to import it"
    } else {
        "Play does not import; import it in edit mode first"
    };
    if first {
        log::warn!("[Asset] {}: not imported yet, and {fix}", source.display());
    }
    ImportError::Parse(format!(
        "{}: no imported copy to play from; {fix}",
        source.display()
    ))
}

/// The one spelling of `source` the held and missing lists compare: `./assets/x.fbx`
/// and `assets/x.fbx` are one entry.
pub(super) fn key(source: &Path) -> PathBuf {
    source
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect()
}
