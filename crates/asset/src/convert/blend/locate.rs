//! Finding Blender, and which Blender it is (#1072).
//!
//! The [`RUSTY_BLENDER`] environment variable names it when set (a non-standard
//! install); otherwise the first `blender` on `PATH`, then the platform's usual
//! install places. Whatever is found is asked its version (`blender --version`)
//! and refused below [`MIN_MAJOR`]: rusty needs Blender 5 or later, and says so
//! with what it found. The search is a plain [`Search`] value, so the tests hand
//! it fakes instead of setting process-wide environment variables.
//!
//! Asking a version starts Blender, which takes most of a second, so the answer
//! (a wrong one included: a broken `blender` on `PATH` is started once, not once
//! per check) is remembered by the executable's path as found, that path
//! canonicalized, and the length and modification time of what it names, so an
//! upgraded Blender is asked again. No answer at all (it could not start, or ran
//! past its limit, as a slow first launch may) is not remembered, and is asked
//! again next time. A version reported and accepted is also kept in the open
//! project's `cache/` ([`super::known`]), so the next process reads it there
//! instead of starting Blender. The executable a search found is remembered too,
//! while the search and that file stay the same and it was not refused, and a
//! newer Blender put earlier on `PATH` after a refusal is found without a
//! restart. What this process remembers is a [`Memo`]: [`Search::find`] uses
//! the process's own, and a test hands [`Search::find_in`] a fresh one, as a new
//! process starts, without emptying another test's.

use super::known::{self, Known};
use crate::ImportError;
use rusty_core::collections::Map;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

/// The environment variable that names the Blender executable to use.
pub const RUSTY_BLENDER: &str = "RUSTY_BLENDER";

/// The oldest Blender major version rusty converts with.
pub const MIN_MAJOR: u32 = 5;

/// The environment variable that sets how long an export may run, in seconds.
pub const RUSTY_BLENDER_TIMEOUT: &str = "RUSTY_BLENDER_TIMEOUT";

/// How long an export may run unless [`RUSTY_BLENDER_TIMEOUT`] says otherwise:
/// long enough for a big scene, short enough that a hung Blender gives the
/// editor and the cache's lock back.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(600);

/// How long `blender --version` may take.
pub(super) const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

/// Where rusty tells people to get Blender.
const DOWNLOAD: &str = "https://www.blender.org/download/";

/// A Blender found, and the version it reported (`5.0.1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Blender {
    pub path: PathBuf,
    pub version: String,
}

/// Where to look for Blender.
#[derive(Clone, PartialEq)]
pub struct Search {
    /// [`RUSTY_BLENDER`]'s value, if set.
    pub explicit: Option<OsString>,
    /// `PATH`'s value, if set.
    pub path: Option<OsString>,
    /// The platform's usual install places, in order.
    pub places: Vec<PathBuf>,
    /// [`RUSTY_BLENDER_TIMEOUT`]'s value, if set.
    pub timeout: Option<OsString>,
    /// The file that keeps the version asked across processes
    /// ([`super::known`]); `None` outside a project, where this process's memo
    /// alone remembers it.
    pub known: Option<PathBuf>,
}

impl Search {
    /// This process's search: its environment and this platform's places.
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

    /// How long an export may run: [`RUSTY_BLENDER_TIMEOUT`]'s seconds, else ten
    /// minutes.
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

    /// The Blender this search finds, version checked.
    pub fn find(&self) -> Result<Blender, ImportError> {
        self.find_in(&MEMO)
    }

    /// [`Search::find`], remembering in `memo`.
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

    /// [`Search::first_found`], remembered while this search, and the file it
    /// found, stay the same.
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

/// The usual install places on this platform, under `home` for a per-user one.
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

/// Why no usable Blender was found: what was, that rusty needs 5+, where to get it.
fn missing(found: &str) -> ImportError {
    ImportError::Parse(format!(
        "{found}; rusty imports .blend files through Blender {MIN_MAJOR} or later: \
         install it from {DOWNLOAD}, or set {RUSTY_BLENDER} to its executable"
    ))
}

/// Whether rusty converts with this version: its leading number (`5` of
/// `5.0.1`) is [`MIN_MAJOR`] or more.
fn accepted(version: &str) -> bool {
    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u32>().ok());
    major.is_some_and(|m| m >= MIN_MAJOR)
}

/// A version asked: the executable's length and mtime when it answered, and
/// the answer.
type Asked = (u64, Option<Duration>, Result<String, ImportError>);

/// An executable as found, and that path canonicalized.
type Key = (PathBuf, PathBuf);

/// What one process remembers: the versions it asked, and its last search.
#[derive(Default)]
pub(super) struct Memo {
    asked: Mutex<Map<Key, Asked>>,
    found: Mutex<Option<(Search, PathBuf)>>,
}

/// This process's [`Memo`].
static MEMO: LazyLock<Memo> = LazyLock::new(Memo::default);

/// The version the Blender at `path` reports, asked once per build of it: from
/// `memo`, else from the `known` file, else from Blender itself.
fn version(path: &Path, known: Option<&Path>, memo: &Memo) -> Result<String, ImportError> {
    // The file a link names gives the length and mtime, so a link moved to
    // another install asks again; the path as found is in the key too, and runs,
    // since a launcher may dispatch on its own name (snap).
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
                    // A version not kept is asked again by the next process: no error.
                    known::write(file, &entry).unwrap_or(());
                }
            }
            answer
        }
    };
    lock().insert(key, (len, modified, answer.clone()));
    answer
}

/// The version `file` keeps for the executable at `key` as it is now, counted as
/// one more read; `None` once it has been read [`known::READS`] times, or when
/// the count cannot be written, so the expiry always holds.
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

/// The version in `blender --version`'s output: the word after the first
/// `Blender` that starts a line.
pub(super) fn parse_version(output: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Blender ")?;
        let word = rest.split_whitespace().next()?;
        word.starts_with(|c: char| c.is_ascii_digit())
            .then(|| word.to_string())
    })
}
