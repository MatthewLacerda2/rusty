//! Whether a cache is its source's conversion yet: the stamp the source converts
//! to now, against the one stored beside the cache (`record`).

use super::depends::{self, Seen};
use super::{record, served_from_cache, unreadable, Converter};
use crate::ImportError;
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum Tool<'a> {
    Inside,
    Found(&'a str),
    Missing(&'a ImportError),
    /// Not asked: Play checks a cache without starting the program (`gate::serve`).
    Unasked,
}

impl<'a> Tool<'a> {
    pub(super) fn of(asked: Option<&'a Result<String, ImportError>>) -> Self {
        match asked {
            None => Self::Inside,
            Some(Ok(version)) => Self::Found(version),
            Some(Err(why)) => Self::Missing(why),
        }
    }

    pub(super) fn unasked(converter: &Converter) -> Self {
        match converter.tool {
            None => Self::Inside,
            Some(_) => Self::Unasked,
        }
    }
}

/// `None` (skips replayed) when the cache is current. With the program unasked or
/// missing, a cache of the unchanged source from any version stands in; a changed
/// source is stale: unasked, the stamp lacks the program's version; missing, `Err`.
pub(super) fn stale(
    converter: &Converter,
    source: &Path,
    out: &Path,
    tool: Tool,
) -> Result<Option<String>, ImportError> {
    let Seen::Read(digest) = depends::digest(source) else {
        if !served_from_cache(false, out) {
            return Err(unreadable(source));
        }
        if let Some(stored) = record::stored(out) {
            record::replay(source, out, &stored);
        }
        return Ok(None);
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
