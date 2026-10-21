//! What a conversion leaves out on purpose (#1172): reported by `refresh`, never
//! the converter, stored under the stamp, and reported again from the cache.

use super::super::{refresh, Converted, Converter, Skip, Staging};
use crate::asset::ImportError;
use crate::logging;
use crate::scripting::ConsoleLogs;
use std::cell::RefCell;
use std::path::Path;

/// A converter that copies its source and skips one part of it.
const SKIPPING: Converter = Converter {
    convert: skipping,
    ..super::COPY
};

fn skipping(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    std::fs::copy(source, &to.glb).map_err(|e| ImportError::Parse(e.to_string()))?;
    let skip = Skip {
        what: "texture `Skin`".into(),
        why: "embeds a `.tga`,\nnot an image".into(),
    };
    Ok(Converted {
        skipped: vec![skip],
    })
}

/// How many forwarded warnings name the skip of `source`.
fn warnings_of(source: &Path) -> usize {
    let line = format!(
        "[Asset] {}: skipped texture `Skin`: embeds a `.tga`, not an image",
        source.display()
    );
    let forwarded = logging::forward(&RefCell::new(ConsoleLogs::new()));
    forwarded.iter().filter(|m| m.starts_with(&line)).count()
}

#[test]
fn a_skip_is_warned_of_on_conversion_and_again_from_the_cache() {
    logging::init();
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();

    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(warnings_of(&source), 1, "warned as it converts");
    let record = std::fs::read_to_string(dir.join("a.src.glb.source")).unwrap();
    assert_eq!(
        record.lines().count(),
        2,
        "the stamp, then one line per skip"
    );

    std::fs::write(&out, "kept").unwrap();
    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "kept",
        "a cache hit"
    );
    assert_eq!(warnings_of(&source), 1, "warned again from the cache");
}
