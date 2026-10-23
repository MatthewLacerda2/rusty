//! Whether a cache is its source's conversion yet: the stamp the source converts
//! to now, against the one stored beside the cache (`record`).

use super::depends::{self, Seen};
use super::{record, unreadable, Converter};
use crate::asset::ImportError;
use std::path::Path;

/// The program a converter runs ([`Converter::tool`]), as one check sees it.
#[derive(Clone, Copy)]
pub(super) enum Tool<'a> {
    /// The converter runs inside the engine.
    Inside,
    /// Found, by the version it reports.
    Found(&'a str),
    /// Not found, or unusable: why.
    Missing(&'a ImportError),
    /// Not asked: Play checks a cache without starting the program (`gate::serve`).
    Unasked,
}

impl<'a> Tool<'a> {
    /// What asking the program said, `None` for a converter inside the engine.
    pub(super) fn of(asked: Option<&'a Result<String, ImportError>>) -> Self {
        match asked {
            None => Self::Inside,
            Some(Ok(version)) => Self::Found(version),
            Some(Err(why)) => Self::Missing(why),
        }
    }

    /// The program of `converter`, not asked.
    pub(super) fn unasked(converter: &Converter) -> Self {
        match converter.tool {
            None => Self::Inside,
            Some(_) => Self::Unasked,
        }
    }
}

/// The stamp `source` converts to, when the cache at `out` is not its conversion
/// yet; `None`, its skips replayed, when it is, or stands in for a missing source.
///
/// With the program unasked or missing, a cache of the unchanged source made by
/// any version of it stands in. A changed source is then stale: unasked, its
/// stamp is returned without the program's version; missing, it is the `Err` of
/// why, since nothing can convert it.
pub(super) fn stale(
    converter: &Converter,
    source: &Path,
    out: &Path,
    tool: Tool,
) -> Result<Option<String>, ImportError> {
    let digest = match depends::digest(source) {
        Seen::Read(digest) => digest,
        _ if out.is_file() => {
            if let Some(stored) = record::stored(out) {
                record::replay(source, out, &stored);
            }
            return Ok(None);
        }
        _ => return Err(unreadable(source)),
    };
    let version = match tool {
        Tool::Found(version) => Some(version),
        _ => None,
    };
    let stamp = record::stamp(converter, digest, version);
    let stored = match tool {
        Tool::Missing(_) | Tool::Unasked => record::current_by_any_tool(out, &stamp),
        Tool::Inside | Tool::Found(_) => record::current(out, &stamp),
    };
    match (stored, tool) {
        (Some(stored), _) => {
            record::replay(source, out, &stored);
            Ok(None)
        }
        (None, Tool::Missing(why)) => Err(why.clone()),
        (None, _) => Ok(Some(stamp)),
    }
}
