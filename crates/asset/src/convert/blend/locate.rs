//! [`RUSTY_BLENDER`] names it when set; otherwise the first `blender` on `PATH`, then the
//! platform's usual install places. Refused below [`MIN_MAJOR`].
//! Asking a version starts Blender (most of a second), so answers are remembered by the
//! executable's path as found and canonicalized plus its length and mtime; an upgrade asks
//! again. No answer at all is not remembered.

use super::known::{self, Known};
use crate::ImportError;
use rusty_core::collections::Map;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

pub const RUSTY_BLENDER: &str = "RUSTY_BLENDER";

pub const MIN_MAJOR: u32 = 5;

pub const RUSTY_BLENDER_TIMEOUT: &str = "RUSTY_BLENDER_TIMEOUT";

const EXPORT_TIMEOUT: Duration = Duration::from_secs(600);

pub(super) const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

const DOWNLOAD: &str = "https://www.blender.org/download/";

#[derive(Clone, Debug, PartialEq)]
pub struct Blender {
    pub path: PathBuf,
    pub version: String,
}

#[derive(Clone, PartialEq)]
pub struct Search {
    pub explicit: Option<OsString>,
    pub path: Option<OsString>,
    pub places: Vec<PathBuf>,
    pub timeout: Option<OsString>,
    pub known: Option<PathBuf>,
}

impl Search {
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        Self {
            explicit: std::env::var_os(RUSTY_BLENDER).filter(|v| !v.is_empty()),
            path: std::env::var_os("PATH"),
            places: places(home.as_deref()),
            timeout: std::env::var_os(RUSTY_BLENDER_TIMEOUT).filter(|v| !v.is_empty()),
            known: known::in_project(Path::new(".")),
        }
    }

    pub fn export_timeout(&self) -> Result<Duration, ImportError> {
        let Some(raw) = &self.timeout else {
            return Ok(EXPORT_TIMEOUT);
        };
        let secs = raw.to_str().and_then(|s| s.trim().parse::<u64>().ok());
        secs.filter(|s| *s > 0)
            .map(Duration::from_secs)
            .ok_or_else(|| {
                ImportError::Parse(format!(
                    "{RUSTY_BLENDER_TIMEOUT} is {}, not a whole number of seconds above 0",
                    raw.to_string_lossy()
                ))
            })
    }

    pub fn find(&self) -> Result<Blender, ImportError> {
        self.find_in(&MEMO)
    }

    pub(super) fn find_in(&self, memo: &Memo) -> Result<Blender, ImportError> {
        let path = match &self.explicit {
            Some(explicit) => {
                let path = PathBuf::from(explicit);
                if !path.is_file() {
                    return Err(missing(&format!(
                        "{RUSTY_BLENDER} names {}, which is not a file",
                        path.display()
                    )));
                }
                path
            }
            None => self.remembered(memo).ok_or_else(|| {
                missing("no Blender was found on PATH or where it is usually installed")
            })?,
        };
        let checked = version(&path, self.known.as_deref(), memo).and_then(|version| {
            if accepted(&version) {
                Ok(Blender { path, version })
            } else {
                Err(missing(&format!("{} is Blender {version}", path.display())))
            }
        });
        if checked.is_err() {
            *memo.found.lock().unwrap_or_else(PoisonError::into_inner) = None;
        }
        checked
    }

    fn remembered(&self, memo: &Memo) -> Option<PathBuf> {
        let lock = || memo.found.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((search, found)) = lock().as_ref() {
            if search == self && found.is_file() {
                return Some(found.clone());
            }
        }
        let found = self.first_found()?;
        *lock() = Some((self.clone(), found.clone()));
        Some(found)
    }

    fn first_found(&self) -> Option<PathBuf> {
        let on_path = self
            .path
            .iter()
            .flat_map(std::env::split_paths)
            .map(|dir| dir.join("blender"));
        on_path
            .chain(self.places.iter().cloned())
            .find(|p| p.is_file())
    }
}

fn places(home: Option<&Path>) -> Vec<PathBuf> {
    let (system, user): (&[&str], &[&str]) = if cfg!(target_os = "macos") {
        (
            &["/Applications/Blender.app/Contents/MacOS/Blender"],
            &["Applications/Blender.app/Contents/MacOS/Blender"],
        )
    } else {
        (
            &[
                "/usr/bin/blender",
                "/usr/local/bin/blender",
                "/var/lib/flatpak/exports/bin/org.blender.Blender",
                "/snap/bin/blender",
            ],
            &[".local/share/flatpak/exports/bin/org.blender.Blender"],
        )
    };
    let user = user.iter().filter_map(|p| Some(home?.join(p)));
    system.iter().map(PathBuf::from).chain(user).collect()
}

fn missing(found: &str) -> ImportError {
    ImportError::Parse(format!(
        "{found}; rusty imports .blend files through Blender {MIN_MAJOR} or later: \
         install it from {DOWNLOAD}, or set {RUSTY_BLENDER} to its executable"
    ))
}

fn accepted(version: &str) -> bool {
    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok());
    major.is_some_and(|m| m >= MIN_MAJOR)
}

type Asked = (u64, Option<Duration>, Result<String, ImportError>);

type Key = (PathBuf, PathBuf);

#[derive(Default)]
pub(super) struct Memo {
    asked: Mutex<Map<Key, Asked>>,
    found: Mutex<Option<(Search, PathBuf)>>,
}

static MEMO: LazyLock<Memo> = LazyLock::new(Memo::default);

fn version(path: &Path, known: Option<&Path>, memo: &Memo) -> Result<String, ImportError> {
    // Length and mtime come from the file a link names, so a moved link asks again; the path as
    // found is in the key too (a launcher such as snap dispatches on its name).
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let meta = std::fs::metadata(&canonical).ok();
    let len = meta.as_ref().map_or(0, std::fs::Metadata::len);
    let modified = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
    let key = (path.to_path_buf(), canonical);
    let lock = || memo.asked.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((l, m, version)) = lock().get(&key) {
        if (*l, *m) == (len, modified) {
            return version.clone();
        }
    }
    let answer = match known.and_then(|file| kept(file, &key, len, modified)) {
        Some(version) => Ok(version),
        None => {
            let answer = super::run::version(path)?;
            if let (Some(file), Ok(version)) = (known, &answer) {
                if accepted(version) {
                    let entry = Known {
                        path: key.0.clone(),
                        canonical: key.1.clone(),
                        len,
                        modified,
                        version: version.clone(),
                        reads: 0,
                    };
                    known::write(file, &entry).unwrap_or(());
                }
            }
            answer
        }
    };
    lock().insert(key, (len, modified, answer.clone()));
    answer
}

fn kept(file: &Path, key: &Key, len: u64, modified: Option<Duration>) -> Option<String> {
    let at = (key.0.as_path(), key.1.as_path());
    let mut entry = known::read(file).filter(|k| k.answers(at, len, modified))?;
    if entry.reads >= known::READS {
        return None;
    }
    entry.reads += 1;
    known::write(file, &entry).ok()?;
    Some(entry.version)
}

pub(super) fn parse_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Blender ")?;
        let word = rest.split_whitespace().next()?;
        word.starts_with(|c: char| c.is_ascii_digit())
            .then(|| word.to_string())
    })
}
