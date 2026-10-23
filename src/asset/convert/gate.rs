//! Whether a stale or missing cache may be converted (#1253).
//!
//! Converting is authoring work (CLAUDE.md, *One format inside*): a conversion is
//! slow, and a cache re-made mid-game would hand new instances of a model geometry
//! its earlier instances don't have. So Play never converts, as Unity's Play never
//! imports: `app::GameWorld` turns conversion off when it enters Play and back on
//! when it leaves, and the standalone player turns it off for good, since a
//! shipped game reads its caches and nothing else.
//!
//! While it is off, [`cached_gltf`](super::cached_gltf) serves the cache as it
//! stands, current or not, and holds back each source it found changed;
//! [`convert_held`] converts them once conversion is on again (the first edit
//! frame after Stop), and the swap's note reloads every entity using the model
//! (#1242). A source with no cache at all fails its load.
//!
//! Process-wide, as the cache and its lock are (per thread under test). The play
//! state lives in `app`, above this module, so `app` sets the switch and this
//! module never reads it.

use super::{cached_gltf, stale, Converter};
use crate::asset::ImportError;
use std::path::{Path, PathBuf};

/// The switch, and the sources found changed while it was off, in the order found.
struct Gate {
    allowed: bool,
    held: Vec<PathBuf>,
}

/// On until Play turns it off.
const ON: Gate = Gate {
    allowed: true,
    held: Vec::new(),
};

#[cfg(not(test))]
static GATE: std::sync::Mutex<Gate> = std::sync::Mutex::new(ON);

/// Run `f` on the gate.
#[cfg(not(test))]
fn with_gate<T>(f: impl FnOnce(&mut Gate) -> T) -> T {
    // Nothing a panic could leave half-written: a bool and a list of paths.
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

/// Allow or refuse conversion, process-wide: refused for Play's duration and for
/// the whole life of a standalone game.
pub fn allow_conversion(allowed: bool) {
    with_gate(|gate| gate.allowed = allowed);
}

/// Whether a stale or missing cache is converted now.
pub fn conversion_allowed() -> bool {
    with_gate(|gate| gate.allowed)
}

/// Convert every source held back while conversion was off, once it is on again.
/// Returns how many were converted; a failure is warned of, as an import's is.
/// A no-op while conversion is off, and cheap when nothing is held.
pub fn convert_held() -> usize {
    let held = with_gate(|gate| {
        if gate.allowed {
            std::mem::take(&mut gate.held)
        } else {
            Vec::new()
        }
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
/// never converted fails, since nothing may convert it now.
pub(super) fn serve(converter: &Converter, source: &Path, out: &Path) -> Result<(), ImportError> {
    if !out.is_file() {
        log::warn!(
            "[Asset] {}: not imported yet, and Play does not import; import it in edit mode first",
            source.display()
        );
        return Err(ImportError::Parse(format!(
            "{}: no imported copy to play from; import it in edit mode first",
            source.display()
        )));
    }
    if stale(converter, source, out)?.is_none() {
        return Ok(());
    }
    let newly_held = with_gate(|gate| {
        let new = !gate.held.iter().any(|p| p == source);
        if new {
            gate.held.push(source.to_path_buf());
        }
        new
    });
    if newly_held {
        log::warn!(
            "[Asset] {}: changed since its import; Play uses the last import until Stop",
            source.display()
        );
    }
    Ok(())
}
