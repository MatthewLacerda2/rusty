//! A conversion swapped into place whole (#1129): a reader of the cache sees the
//! old set or the new one, and a failure leaves the old one as it was.

use super::super::{refresh, textures_dirs, Converter, Staging};
use crate::asset::ImportError;
use std::path::Path;

/// A converter whose `.glb` references a texture it writes: the glTF is the
/// source's text and its textures folder's name, the texture the same text.
const TEXTURED: Converter = Converter {
    extension: "src",
    name: "textured",
    version: 1,
    convert: textured,
};

fn textured(source: &Path, to: &Staging) -> Result<(), ImportError> {
    // As a reader in another process would, mid re-conversion: the old set is whole.
    if to.out.is_file() {
        read_whole(&to.out);
    }
    let text = std::fs::read_to_string(source).unwrap();
    std::fs::create_dir_all(&to.textures).unwrap();
    std::fs::write(to.textures.join("t.png"), &text).unwrap();
    let folder = to.textures.file_name().unwrap().to_string_lossy();
    std::fs::write(&to.glb, format!("{text}\n{folder}")).unwrap();
    Ok(())
}

/// A converter that writes half a `.glb`, then fails.
const FAILING: Converter = Converter {
    convert: failing,
    ..TEXTURED
};

fn failing(_: &Path, to: &Staging) -> Result<(), ImportError> {
    std::fs::write(&to.glb, "half").unwrap();
    Err(ImportError::Parse("broken".into()))
}

/// What the cached `.glb` at `out` says, after checking the texture it names is
/// there and agrees with it.
fn read_whole(out: &Path) -> String {
    let glb = std::fs::read_to_string(out).unwrap();
    let (text, folder) = glb.split_once('\n').expect("a reference");
    let texture = out.parent().unwrap().join(folder).join("t.png");
    assert_eq!(
        std::fs::read_to_string(texture).unwrap(),
        text,
        "a torn set"
    );
    text.to_string()
}

/// Every name in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_reader_during_a_reconversion_sees_the_old_set_whole() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("cache/a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&TEXTURED, &source, &out).unwrap();
    assert_eq!(read_whole(&out), "one");

    std::fs::write(&source, "two").unwrap();
    refresh(&TEXTURED, &source, &out).unwrap();
    assert_eq!(read_whole(&out), "two");
    // The old textures folder went once the new `.glb` was in; nothing staged is left.
    assert_eq!(textures_dirs(&out).len(), 1);
    assert_eq!(
        names(&dir.join("cache")).len(),
        3,
        "{:?}",
        names(&dir.join("cache"))
    );
}

#[test]
fn a_failing_converter_leaves_the_old_cache_intact() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&TEXTURED, &source, &out).unwrap();
    let before = names(&dir);

    std::fs::write(&source, "two").unwrap();
    assert!(refresh(&FAILING, &source, &out).is_err());
    assert_eq!(read_whole(&out), "one");
    assert_eq!(names(&dir), before, "the staging folder is removed");
}

#[test]
fn a_reconversion_clears_what_a_crash_or_an_older_converter_left() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    let leftovers = [".a.src.glb.999.0.staging", "a.src.glb.textures"];
    for leftover in leftovers {
        std::fs::create_dir_all(dir.join(leftover).join("junk")).unwrap();
    }
    // Not this cache's: another source whose name starts like it.
    std::fs::create_dir_all(dir.join("a.src.glb.b.src.glb.textures")).unwrap();

    refresh(&TEXTURED, &source, &out).unwrap();
    let names = names(&dir);
    for leftover in leftovers {
        assert!(!names.iter().any(|n| n == leftover), "{leftover} left");
    }
    assert!(names.iter().any(|n| n == "a.src.glb.b.src.glb.textures"));
}

#[test]
fn a_lost_stamp_reconverts_over_its_own_textures_folder() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&TEXTURED, &source, &out).unwrap();
    std::fs::remove_file(dir.join("a.src.glb.source")).unwrap();

    refresh(&TEXTURED, &source, &out).unwrap();
    assert_eq!(read_whole(&out), "one");
    assert_eq!(names(&dir).len(), 4, "{:?}", names(&dir));
}
