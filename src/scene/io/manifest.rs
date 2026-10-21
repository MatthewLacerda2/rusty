//! src/scene/io/manifest.rs — the seed manifest (#746).
//!
//! The open project (`core::project`) is seeded from the engine's bundled
//! defaults. Seeding only when a file is missing let a workspace go stale: an engine
//! fix to the default scene or `bot.lua` never reached it. The manifest records the
//! hash of every file as it was seeded, which is the proof needed to refresh it:
//!
//! - missing → written;
//! - identical to the bundled version → up to date (and recorded, even if the
//!   manifest never saw it);
//! - unchanged since seeding (its hash is the recorded one) → refreshed;
//! - anything else is the user's edit, or a file seeded before the manifest existed:
//!   kept, with one line saying a newer default exists. Never overwritten without
//!   proof.
//!
//! Generic over the file: a caller hands in the destination and the bundled bytes,
//! so a rewritten default (a new scene, a new script) rides the same path.
//!
//! Tests and tools seed from many processes at once, so every write is staged and
//! renamed into place, and saving re-reads the manifest and overlays only the entries
//! this run touched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::fs;
use crate::core::hash::fnv1a64;

/// The manifest's place, relative to the project root.
pub const SEED_MANIFEST_PATH: &str = ".seeded";

/// What [`SeedManifest::seed`] did with one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOutcome {
    /// The file was missing and is now the bundled version.
    Written,
    /// The file already equals the bundled version.
    UpToDate,
    /// The file was unchanged since seeding and now carries the newer bundled version.
    Refreshed,
    /// The file differs from both the bundled version and the seeded one: kept as is.
    Kept,
}

/// The recorded seed hashes, keyed by the destination path as the caller names it.
pub struct SeedManifest {
    path: PathBuf,
    entries: BTreeMap<String, u64>,
    touched: BTreeMap<String, u64>,
}

impl SeedManifest {
    /// Read the manifest at `path`; a missing or unreadable one is empty.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = read_entries(&path);
        Self {
            path,
            entries,
            touched: BTreeMap::new(),
        }
    }

    /// Seed `dest` with `bundled`, under the rules in the module docs. Prints one
    /// line when a file is refreshed or kept; `hint` says how to take the new default.
    pub fn seed(&mut self, dest: &Path, bundled: &[u8], hint: &str) -> SeedOutcome {
        let key = dest.to_string_lossy().into_owned();
        let want = fnv1a64(bundled);
        let outcome = match std::fs::read(dest) {
            Err(_) => SeedOutcome::Written,
            Ok(current) if fnv1a64(&current) == want => SeedOutcome::UpToDate,
            Ok(current) if self.entries.get(&key) == Some(&fnv1a64(&current)) => {
                SeedOutcome::Refreshed
            }
            Ok(_) => SeedOutcome::Kept,
        };
        match outcome {
            SeedOutcome::Kept => {
                eprintln!(
                    "[Seed] {key}: a newer engine default exists; your copy is kept ({hint})"
                );
                return outcome;
            }
            SeedOutcome::Written | SeedOutcome::Refreshed => {
                if let Err(e) = write_atomic(dest, bundled) {
                    eprintln!("[Seed] {key}: seeding failed: {e}");
                    return outcome;
                }
                if outcome == SeedOutcome::Refreshed {
                    eprintln!("[Seed] {key}: refreshed to the engine's current default");
                }
            }
            SeedOutcome::UpToDate => {}
        }
        if self.entries.get(&key) != Some(&want) {
            self.entries.insert(key.clone(), want);
            self.touched.insert(key, want);
        }
        outcome
    }

    /// Write back the entries this run changed, merged over what is on disk now.
    pub fn save(&self) -> Result<(), String> {
        if self.touched.is_empty() {
            return Ok(());
        }
        let mut merged = read_entries(&self.path);
        merged.extend(self.touched.iter().map(|(k, v)| (k.clone(), *v)));
        let text: String = merged
            .iter()
            .map(|(k, h)| format!("{h:016x} {k}\n"))
            .collect();
        write_atomic(&self.path, text.as_bytes())
    }
}

/// One `<hash hex> <path>` line per file; malformed lines are skipped.
fn read_entries(path: &Path) -> BTreeMap<String, u64> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let (hash, key) = line.split_once(' ')?;
            Some((key.to_string(), u64::from_str_radix(hash, 16).ok()?))
        })
        .collect()
}

/// Write `bytes` at `dest` atomically ([`fs::write_atomic`]), creating its folder:
/// a reader in another process sees the whole file or the old one, never half of it.
fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    fs::write_atomic(dest, bytes).map_err(|e| format!("{}: {e}", dest.display()))
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
