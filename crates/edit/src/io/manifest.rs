use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusty_core::fs;
use rusty_core::hash::fnv1a64;
use rusty_core::paths;

pub const SEED_MANIFEST_PATH: &str = ".seeded";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOutcome {
    Written,
    UpToDate,
    Refreshed,
    Kept,
    Deleted,
}

pub struct SeedManifest {
    path: PathBuf,
    entries: BTreeMap<String, u64>,
    touched: BTreeMap<String, u64>,
}

impl SeedManifest {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = read_entries(&path);
        Self {
            path,
            entries,
            touched: BTreeMap::new(),
        }
    }

    pub fn seed(&mut self, dest: &Path, bundled: &[u8], hint: &str) -> SeedOutcome {
        let key = paths::to_slash(dest);
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
                if let Err(e) = make_parent(dest)
                    .and_then(|()| fs::written::write(dest, bundled).map_err(|e| e.to_string()))
                {
                    eprintln!("[Seed] {key}: seeding failed: {e}");
                    return outcome;
                }
                if outcome == SeedOutcome::Refreshed {
                    eprintln!("[Seed] {key}: refreshed to the engine's current default");
                }
            }
            SeedOutcome::UpToDate => {}
            SeedOutcome::Deleted => return outcome,
        }
        if self.entries.get(&key) != Some(&want) {
            self.entries.insert(key.clone(), want);
            self.touched.insert(key, want);
        }
        outcome
    }

    pub fn seed_once(&mut self, dest: &Path, bundled: &[u8], hint: &str) -> SeedOutcome {
        if !dest.exists() && self.entries.contains_key(&paths::to_slash(dest)) {
            return SeedOutcome::Deleted;
        }
        self.seed(dest, bundled, hint)
    }

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
        make_parent(&self.path)?;
        // lint: allow(asset-write) the seed manifest's own record, read only by the next seed
        fs::write_atomic(&self.path, text.as_bytes())
            .map_err(|e| format!("{}: {e}", self.path.display()))
    }
}

fn read_entries(path: &Path) -> BTreeMap<String, u64> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let (hash, key) = line.split_once(' ')?;
            Some((key.to_string(), u64::from_str_radix(hash, 16).ok()?))
        })
        .collect()
}

fn make_parent(dest: &Path) -> Result<(), String> {
    match dest.parent() {
        Some(parent) => {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))
        }
        None => Ok(()),
    }
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
