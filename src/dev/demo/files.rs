//! Where demos go: `saved/demos/<scene>-<UTC time>.demo`, the newest N kept (#1581).
//!
//! The platform layer names the file: the wall-clock time is in the name only and
//! never reaches the sim.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The project folder demos are written to (inside the git-ignored `saved/`).
pub const DEMOS_DIR: &str = "saved/demos";

/// The extension every demo file carries.
pub const EXTENSION: &str = "demo";

/// Where a recorder writes.
pub enum Destination {
    /// A folder of demos, one per Play session, pruned to the project's `keep`.
    Folder(PathBuf),
    /// One file, rewritten by each Play session (tests, a chosen path).
    File(PathBuf),
}

impl Destination {
    /// The path the next demo of `scene` goes to, after pruning the folder so it
    /// holds at most `keep` with the new one. `None` when the project keeps none.
    pub fn next(&self, scene: &str, keep: usize) -> Option<PathBuf> {
        match self {
            Self::File(path) => Some(path.clone()),
            Self::Folder(_) if keep == 0 => None,
            Self::Folder(dir) => {
                if let Err(err) = std::fs::create_dir_all(dir) {
                    log::warn!("demos: cannot create {}: {err}", dir.display());
                    return None;
                }
                prune(dir, keep - 1);
                Some(fresh_name(dir, scene))
            }
        }
    }
}

/// Delete the oldest demos in `dir` until at most `keep` are left.
fn prune(dir: &Path, keep: usize) {
    let mut demos = demos_in(dir);
    let excess = demos.len().saturating_sub(keep);
    for path in demos.drain(..excess) {
        let _ = std::fs::remove_file(path);
    }
}

/// The demos in `dir`, oldest first (by modification time, then name).
pub fn demos_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut demos: Vec<(SystemTime, PathBuf)> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == EXTENSION))
        .map(|path| {
            let modified = path.metadata().and_then(|m| m.modified());
            (modified.unwrap_or(UNIX_EPOCH), path)
        })
        .collect();
    demos.sort();
    demos.into_iter().map(|(_, path)| path).collect()
}

/// `<dir>/<scene stem>-<YYYYMMDD-HHMMSS>.demo`, with `-2`, `-3`… when taken.
fn fresh_name(dir: &Path, scene: &str) -> PathBuf {
    let stem = Path::new(scene)
        .file_stem()
        .map_or_else(|| "untitled".into(), |s| s.to_string_lossy());
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = format!("{stem}-{}", utc_stamp(seconds));
    let mut path = dir.join(format!("{base}.{EXTENSION}"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base}-{n}.{EXTENSION}"));
        n += 1;
    }
    path
}

/// `YYYYMMDD-HHMMSS` in UTC for `seconds` since the Unix epoch.
fn utc_stamp(seconds: u64) -> String {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let (h, m, s) = (rest / 3_600, rest % 3_600 / 60, rest % 60);
    format!("{year:04}{month:02}{day:02}-{h:02}{m:02}{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stamp_is_the_utc_date_and_time() {
        assert_eq!(utc_stamp(0), "19700101-000000");
        assert_eq!(utc_stamp(1_793_311_445), "20261029-220405");
        assert_eq!(utc_stamp(951_782_400), "20000229-000000");
    }

    #[test]
    fn a_folder_keeps_the_newest_and_none_when_keep_is_zero() {
        let dir = crate::test_temp::dir();
        let folder = Destination::Folder(dir.clone());
        assert_eq!(folder.next("a.scene", 0), None);
        for i in 0..4 {
            let path = folder.next("assets/scenes/main.scene", 3).unwrap();
            assert!(path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("main-"));
            std::fs::write(&path, format!("{i}")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let left = demos_in(&dir);
        assert_eq!(left.len(), 3, "the oldest went: {left:?}");
        assert_eq!(std::fs::read_to_string(&left[0]).unwrap(), "1");
    }
}
