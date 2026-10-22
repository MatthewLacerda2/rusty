//! The files a conversion looked at beside its source (#1189): the cache is
//! current only while each reads as it did, so one appearing, changing or
//! vanishing re-converts.

use super::super::{refresh, Converted, Converter, Dependency, Skip, Staging};
use crate::asset::ImportError;
use std::path::{Path, PathBuf};

/// A converter that copies its source and appends the texture beside it
/// (`<source>.tex`), or skips it when it is missing.
const LOOKING: Converter = Converter {
    convert: looking,
    ..super::COPY
};

fn texture_of(source: &Path) -> PathBuf {
    source.with_extension("tex")
}

fn looking(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let texture = texture_of(source);
    let looked = Dependency::look(&texture);
    let read = |path: &Path| std::fs::read(path).map_err(|e| ImportError::Parse(e.to_string()));
    let mut glb = read(source)?;
    let mut skipped = Vec::new();
    if looked.found() {
        glb.extend(read(&texture)?);
    } else {
        skipped.push(Skip {
            what: "texture".into(),
            why: "missing".into(),
        });
    }
    std::fs::write(&to.glb, glb).map_err(|e| ImportError::Parse(e.to_string()))?;
    Ok(Converted {
        skipped,
        depends_on: vec![looked],
    })
}

/// Refresh `out`, returning what the cache holds and whether its stamp file
/// records a skip.
fn refreshed(source: &Path, out: &Path) -> (String, bool) {
    refresh(&LOOKING, source, out).unwrap();
    let record = std::fs::read_to_string(super::super::stamp_path(out)).unwrap();
    let skips = record.lines().any(|l| l.starts_with("skipped "));
    (std::fs::read_to_string(out).unwrap(), skips)
}

#[test]
fn a_texture_appearing_changing_or_vanishing_reconverts() {
    let dir = crate::test_temp::dir().join("depends");
    std::fs::create_dir_all(&dir).unwrap();
    // A space and a `%`: the path is stored encoded and must read back exactly.
    let (source, out) = (dir.join("a b%.src"), dir.join("a b%.src.glb"));
    let texture = texture_of(&source);
    std::fs::write(&source, "one").unwrap();
    assert_eq!(refreshed(&source, &out), ("one".into(), true), "missing");

    std::fs::write(&out, "kept").unwrap();
    assert_eq!(refreshed(&source, &out).0, "kept", "still missing: current");

    std::fs::write(&texture, "+red").unwrap();
    assert_eq!(refreshed(&source, &out), ("one+red".into(), false), "added");

    std::fs::write(&out, "kept").unwrap();
    assert_eq!(refreshed(&source, &out).0, "kept", "unchanged: current");

    std::fs::write(&texture, "+blue").unwrap();
    assert_eq!(
        refreshed(&source, &out),
        ("one+blue".into(), false),
        "changed"
    );

    std::fs::remove_file(&texture).unwrap();
    assert_eq!(refreshed(&source, &out), ("one".into(), true), "removed");
}

#[test]
fn a_missing_source_keeps_its_cache_whatever_its_files_read() {
    let dir = crate::test_temp::dir().join("depends_shipped");
    std::fs::create_dir_all(&dir).unwrap();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    std::fs::write(texture_of(&source), "+red").unwrap();
    refresh(&LOOKING, &source, &out).unwrap();

    // A game shipped without its sources or their textures.
    std::fs::remove_file(&source).unwrap();
    std::fs::remove_file(texture_of(&source)).unwrap();
    assert_eq!(refreshed(&source, &out), ("one+red".into(), false));
}
