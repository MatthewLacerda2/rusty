//! The model imports one rehydrate pass has looked up, by path.
//!
//! [`rusty_asset::imported`] answers from its cache, but each call still brings the
//! file's conversion up to date and reads its import settings before the hit; a
//! level instancing one model hundreds of times paid that once per instance. A
//! pass (a load, a Play snapshot's restore) asks once per distinct file instead:
//! nothing it does writes a model, so the answer cannot change within it.

use std::collections::BTreeMap;
use std::path::Path;

use rusty_asset::{ImportError, Imported};

/// Each file's import (or why it failed), looked up on first use.
#[derive(Default)]
pub(crate) struct Imports(BTreeMap<String, Result<Imported, ImportError>>);

impl Imports {
    /// The import of the model file at `path`, looked up once per pass.
    pub(crate) fn of(&mut self, path: &str) -> Result<Imported, ImportError> {
        self.0
            .entry(path.to_string())
            .or_insert_with(|| rusty_asset::imported(Path::new(path)))
            .clone()
    }
}
