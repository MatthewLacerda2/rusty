//! A conversion made again only because a file it looked at changed writes a
//! textures folder of its own (#1072): a `.blend`'s linked library changes the
//! bytes of an image whose name stays, which must never be rewritten in the
//! folder the live `.glb` reads.

use super::super::{refresh, textures_dirs, Converted, Converter, Dependency, Staging};
use crate::asset::ImportError;
use std::path::{Path, PathBuf};

/// A converter that copies its source, and writes the file beside it (its
/// "library") into the textures folder as `t.png`, under the same name whatever
/// it holds.
const LINKING: Converter = Converter {
    convert: linking,
    ..super::COPY
};

fn library(source: &Path) -> PathBuf {
    source.with_extension("lib")
}

fn linking(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let fail = |e: std::io::Error| ImportError::Parse(e.to_string());
    std::fs::copy(source, &to.glb).map_err(fail)?;
    std::fs::create_dir(&to.textures).map_err(fail)?;
    std::fs::copy(library(source), to.textures.join("t.png")).map_err(fail)?;
    Ok(Converted {
        depends_on: vec![Dependency::look(&library(source))?],
        ..Converted::default()
    })
}

/// The one textures folder beside `out`, and what its `t.png` holds.
fn folder(out: &Path) -> (PathBuf, String) {
    let [dir] = textures_dirs(out).try_into().expect("one textures folder");
    let held = std::fs::read_to_string(dir.join("t.png")).unwrap();
    (dir, held)
}

#[test]
fn a_changed_library_converts_into_a_folder_of_its_own() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "model").unwrap();
    std::fs::write(library(&source), "blue").unwrap();
    refresh(&LINKING, &source, &out).unwrap();
    let (first, held) = folder(&out);
    assert_eq!(held, "blue");

    std::fs::write(library(&source), "red").unwrap();
    refresh(&LINKING, &source, &out).unwrap();
    let (second, held) = folder(&out);
    assert_eq!(held, "red");
    assert_ne!(first, second, "never rewritten in place");

    std::fs::write(library(&source), "blue").unwrap();
    refresh(&LINKING, &source, &out).unwrap();
    assert_eq!(folder(&out).1, "blue", "and back");
}
