//! src/asset/convert/fingerprint/ — a converter's output can't change without a
//! bump of its [`Converter::version`] (#1286).
//!
//! The version is what tells the import cache to re-make its files: a change of
//! output left at the old version keeps every cache made before it current, so a
//! project holds the old conversion until its source is edited. A sentence asking
//! for the bump was missed twice in one batch (#1130, #1280), so a test holds it.
//!
//! Each converter converts each of its fixtures (`tests/fixtures/models/` files of
//! its extension), and the output is hashed: the `.glb`'s bytes, then each file
//! written beside it, by name and bytes. [`FINGERPRINTS`] holds the hashes,
//! blessed per converter, version, fixture and program ([`Converter::tool`]: a
//! different Blender may write differently). [`table::check`] fails when a hash
//! differs at the blessed version, naming the converter and the bump;
//! [`table::bless`] refuses the same, so re-blessing never stands in for it.
//!
//! **Only deterministic output is hashed.** Each fixture is converted twice into
//! fresh folders and the two must be byte-identical before either is hashed. The
//! conversions run outside the cache (no stamp, no tagged textures folder), with
//! [`Staging::out`] placed in the fixtures folder, so a texture on disk is named
//! by the same relative path on every machine. The hashes are Linux's (CI's
//! suite and `blender` jobs, x86_64): another platform's maths library may round
//! a float differently, so the check runs on Linux only.

pub(super) mod table;

use super::{Converter, Staging};
use crate::core::hash;
use std::path::{Path, PathBuf};
use table::{Now, Row};

/// The committed table, relative to the crate root.
pub const FINGERPRINTS: &str = "src/asset/convert/fingerprints.txt";

/// Set to re-bless [`FINGERPRINTS`] instead of checking it (`make bless-converters`).
const BLESS: &str = "RUSTY_BLESS_CONVERTERS";

/// Check (or, with [`BLESS`] set, re-bless) every converter `which` picks.
pub fn gate(which: impl Fn(&Converter) -> bool) {
    let path = crate_root().join(crate::core::paths::native(FINGERPRINTS));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut rows: Vec<Row> = table::parse(&text).unwrap_or_else(|e| panic!("{FINGERPRINTS}: {e}"));
    let bless = std::env::var_os(BLESS).is_some();
    for converter in super::CONVERTERS.iter().filter(|c| which(c)) {
        let now = now(converter);
        if bless {
            rows = table::bless(&rows, &now).unwrap_or_else(|e| panic!("{e}"));
        } else if let Err(e) = table::check(&rows, &now) {
            panic!("{e}\n(the table is {FINGERPRINTS}; docs/testing.md, *Converter fingerprints*)");
        }
    }
    if bless {
        std::fs::write(&path, table::format(&rows)).expect("write the fingerprints");
    }
}

/// What `converter` writes now, for each of its fixtures.
fn now(converter: &Converter) -> Now {
    let tool = converter.tool.map_or_else(
        || "-".to_string(),
        |identify| identify().unwrap_or_else(|e| panic!("{}: {e}", converter.name)),
    );
    let hashes = fixtures(converter.extension)
        .iter()
        .map(|source| {
            let name = file_name(source);
            (
                name.clone(),
                format!("{:016x}", deterministic(converter, source, &name)),
            )
        })
        .collect();
    Now {
        converter: converter.name.to_string(),
        version: converter.version,
        tool,
        hashes,
    }
}

/// The hash of `source`'s conversion, once two conversions agree byte for byte.
fn deterministic(converter: &Converter, source: &Path, name: &str) -> u64 {
    let [first, second] = ["a", "b"].map(|run| convert(converter, source, name, run));
    assert!(
        first == second,
        "the {} converter writes {name} differently twice: its output is not deterministic",
        converter.name
    );
    hash::fnv1a64(&first)
}

/// `source` converted into a fresh folder: the `.glb`'s bytes, then each file
/// beside it as name, length and bytes, by name.
fn convert(converter: &Converter, source: &Path, name: &str, run: &str) -> Vec<u8> {
    let dir = crate::test_temp::dir().join(format!("{name}.{run}"));
    std::fs::create_dir_all(&dir).expect("a conversion folder");
    let to = Staging {
        glb: dir.join(format!("{name}.glb")),
        textures: dir.join(format!("{name}.textures")),
        out: source.with_file_name(format!("{name}.glb")),
    };
    (converter.convert)(source, &to).unwrap_or_else(|e| panic!("{name} converts: {e}"));
    let mut bytes = std::fs::read(&to.glb).expect("the converter wrote its .glb");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&to.textures)
        .map(|entries| {
            entries
                .map(|e| e.expect("a textures entry").path())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for file in files {
        let content = std::fs::read(&file).expect("a written texture");
        bytes.extend(file_name(&file).as_bytes());
        bytes.extend((content.len() as u64).to_le_bytes());
        bytes.extend(content);
    }
    bytes
}

/// The fixtures of `extension`, by name.
fn fixtures(extension: &str) -> Vec<PathBuf> {
    let dir = crate_root().join(crate::core::paths::native("tests/fixtures/models"));
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the model fixtures")
        .map(|e| e.expect("a fixtures entry").path())
        .filter(|p| crate::core::paths::extension(p) == extension)
        .collect();
    found.sort();
    found
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[cfg(test)]
mod tests;
