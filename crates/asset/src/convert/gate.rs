//! Conversion is refused while a [`PlayHold`] is taken: a cache re-made mid-game
//! would give new instances geometry the old ones lack (Unity refreshes before Play).
//! Changed sources are held for [`convert_held`].

use super::{cache_location, cached_gltf, converter_for, record, stale, Converter, Tool};
use crate::ImportError;
use rusty_core::fs::written::Stale;
use std::path::{Component, Path, PathBuf};

struct Gate {
    plays: usize,
    standalone: bool,
    held: Vec<PathBuf>,
    missing: Vec<PathBuf>,
}

impl Gate {
    fn allowed(&self) -> bool {
        self.plays == 0
    }

    fn uses(&self) -> &'static str {
        if self.standalone {
            "the game uses the last import"
        } else {
            "Play uses the last import until Stop"
        }
    }
}

const ON: Gate = Gate {
    plays: 0,
    standalone: false,
    held: Vec::new(),
    missing: Vec::new(),
};

// Per thread in every build (integration tests lack `cfg(test)`): a world loads on its
// ticking thread, and a load elsewhere must take its own hold.
thread_local! {
    static GATE: std::cell::RefCell<Gate> = const { std::cell::RefCell::new(ON) };
}

fn with_gate<T>(f: impl FnOnce(&mut Gate) -> T) -> T {
    GATE.with(|gate| f(&mut gate.borrow_mut()))
}

/// Not `Send`: it counts on its thread's gate.
#[must_use = "conversion is refused only while the hold lives"]
pub struct PlayHold(std::marker::PhantomData<*const ()>);

impl PlayHold {
    pub fn take() -> Self {
        with_gate(|gate| gate.plays += 1);
        Self(std::marker::PhantomData)
    }
}

impl Drop for PlayHold {
    fn drop(&mut self) {
        with_gate(|gate| gate.plays -= 1);
    }
}

pub fn mark_standalone() {
    with_gate(|gate| gate.standalone = true);
}

pub(crate) fn is_standalone() -> bool {
    with_gate(|gate| gate.standalone)
}

pub fn conversion_allowed() -> bool {
    with_gate(|gate| gate.allowed())
}

/// A standalone game compresses no image: it plays from its export's copies.
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
    if !is_standalone() {
        crate::texture_import::refresh_all(root);
    }
    sources.len()
}

pub fn convertible_sources(root: &Path) -> Vec<PathBuf> {
    let mut sources = Vec::new();
    crate::manifest::walk(root, &mut |path| {
        if converter_for(path).is_some() {
            sources.push(path.to_path_buf());
        }
    });
    sources.sort();
    sources
}

/// Only sources `changed` touches: checking all would rerun a failing one each frame.
pub fn refresh_changed(root: &Path, changed: Vec<String>) -> usize {
    if !conversion_allowed() {
        return 0;
    }
    let changed = Stale::new(changed);
    crate::texture_import::queue_changed(root, &changed);
    let touched = touched(root, &changed);
    let mut converted = 0;
    for source in touched {
        match cached_gltf(&source) {
            Ok(_) => converted += 1,
            Err(e) => log::warn!("[Asset] {}: {e}", source.display()),
        }
    }
    converted
}

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

/// Never starts a converter's program, not even for its version.
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

pub(super) fn key(source: &Path) -> PathBuf {
    source
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect()
}
