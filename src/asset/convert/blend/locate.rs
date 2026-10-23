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
//! is remembered by the executable's path, length and modification time: a check
//! of a current cache costs a few `stat`s, and an upgraded Blender is asked again.

use crate::asset::ImportError;
use crate::core::collections::Map;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

/// The environment variable that names the Blender executable to use.
pub const RUSTY_BLENDER: &str = "RUSTY_BLENDER";

/// The oldest Blender major version rusty converts with.
pub const MIN_MAJOR: u32 = 5;

/// Where rusty tells people to get Blender.
const DOWNLOAD: &str = "https://www.blender.org/download/";

/// A Blender found, and the version it reported (`5.0.1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Blender {
    pub path: PathBuf,
    pub version: String,
}

/// Where to look for Blender.
pub struct Search {
    /// [`RUSTY_BLENDER`]'s value, if set.
    pub explicit: Option<OsString>,
    /// `PATH`'s value, if set.
    pub path: Option<OsString>,
    /// The platform's usual install places, in order.
    pub places: Vec<PathBuf>,
}

impl Search {
    /// This process's search: its environment and this platform's places.
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        Self {
            explicit: std::env::var_os(RUSTY_BLENDER).filter(|v| !v.is_empty()),
            path: std::env::var_os("PATH"),
            places: places(home.as_deref()),
        }
    }

    /// The Blender this search finds, version checked.
    pub fn find(&self) -> Result<Blender, ImportError> {
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
            None => self.first_found().ok_or_else(|| {
                missing("no Blender was found on PATH or where it is usually installed")
            })?,
        };
        let version = version(&path)?;
        let major = major(&version);
        if major.is_none_or(|m| m < MIN_MAJOR) {
            return Err(missing(&format!("{} is Blender {version}", path.display())));
        }
        Ok(Blender { path, version })
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

/// The leading number of a version (`5` of `5.0.1`).
fn major(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

/// A version asked: the executable's length and mtime when it answered, and
/// the answer.
type Asked = (u64, Option<Duration>, String);

/// Versions already asked, by executable path.
static ASKED: LazyLock<Mutex<Map<PathBuf, Asked>>> = LazyLock::new(Mutex::default);

/// The version the Blender at `path` reports, asked once per build of it.
fn version(path: &Path) -> Result<String, ImportError> {
    let meta = std::fs::metadata(path).ok();
    let len = meta.as_ref().map_or(0, std::fs::Metadata::len);
    let modified = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
    let lock = || ASKED.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some((l, m, version)) = lock().get(path) {
        if (*l, *m) == (len, modified) {
            return Ok(version.clone());
        }
    }
    let version = super::run::version(path)?;
    lock().insert(path.to_path_buf(), (len, modified, version.clone()));
    Ok(version)
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
