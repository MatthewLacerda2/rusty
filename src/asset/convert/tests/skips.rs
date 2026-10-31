//! What a conversion leaves out on purpose (#1172): reported by `refresh`, never
//! the converter, stored under the stamp, and replayed from the cache once per
//! process.

use super::super::{record, refresh, Converted, Converter, Skip, Staging};
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
        ..Converted::default()
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

/// `source` converted into `out` (warning once, stored under the stamp), with the
/// cache then marked so a hit is visible.
fn converted(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    logging::init();
    let dir = crate::test_temp::dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(warnings_of(&source), 1, "warned as it converts");
    let record = std::fs::read_to_string(dir.join("a.src.glb.source")).unwrap();
    assert_eq!(
        record.lines().count(),
        3,
        "the stamp, the textures folder's tag, then one line per skip"
    );
    std::fs::write(&out, "kept").unwrap();
    (source, out)
}

#[test]
fn a_cache_hit_replays_its_skips_once_per_process() {
    let (source, out) = converted("skips_replay");
    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "kept",
        "a cache hit"
    );
    assert_eq!(warnings_of(&source), 0, "this process already warned");

    record::forget(&out); // a fresh process
    refresh(&SKIPPING, &source, &out).unwrap();
    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(warnings_of(&source), 1, "two hits warn once");
}

#[test]
fn a_cache_standing_in_for_a_missing_source_replays_its_skips() {
    let (source, out) = converted("skips_shipped");
    std::fs::remove_file(&source).unwrap();
    record::forget(&out);
    refresh(&SKIPPING, &source, &out).unwrap();
    assert_eq!(warnings_of(&source), 1);
}

#[test]
fn a_reconversion_elsewhere_for_a_changed_file_warns_of_its_new_skips() {
    use super::super::Dependency;
    use super::depends::{texture_of, LOOKING};
    logging::init();
    let dir = crate::test_temp::dir().join("skips_elsewhere");
    std::fs::create_dir_all(&dir).unwrap();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    std::fs::write(texture_of(&source), "+red").unwrap();
    refresh(&LOOKING, &source, &out).unwrap(); // nothing skipped, warned of
    logging::forward(&RefCell::new(ConsoleLogs::new()));

    // Another process converts again once the texture is gone: same stamp.
    std::fs::remove_file(texture_of(&source)).unwrap();
    let converted = Converted {
        skipped: vec![Skip {
            what: "texture".into(),
            why: "missing".into(),
        }],
        depends_on: vec![Dependency::look(&texture_of(&source)).unwrap()],
    };
    let stamp = record::stamp(&LOOKING, crate::core::hash::content(b"one"), None);
    let stamp_file = record::record(&stamp, &record::folder_tag(&out, &stamp), &converted);
    std::fs::write(super::super::stamp_path(&out), stamp_file).unwrap();

    refresh(&LOOKING, &source, &out).unwrap();
    let line = format!("[Asset] {}: skipped texture: missing", source.display());
    let forwarded = logging::forward(&RefCell::new(ConsoleLogs::new()));
    assert_eq!(forwarded.iter().filter(|m| m.starts_with(&line)).count(), 1);
}
