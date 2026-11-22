pub(super) mod table;

use super::{Converter, Staging};
use rusty_core::hash;
use std::path::{Path, PathBuf};
use table::{Now, Row};

pub const FINGERPRINTS: &str = "crates/asset/src/convert/fingerprints.txt";

const BLESS: &str = "RUSTY_BLESS_CONVERTERS";

pub fn gate(which: impl Fn(&Converter) -> bool) {
    let path = rusty_core::project::checkout().join(FINGERPRINTS);
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

fn deterministic(converter: &Converter, source: &Path, name: &str) -> u64 {
    let [first, second] = ["a", "b"].map(|run| convert(converter, source, name, run));
    assert!(
        first == second,
        "the {} converter writes {name} differently twice: its output is not deterministic",
        converter.name
    );
    // FNV-1a, not `hash::content`: FINGERPRINTS holds these hashes, and
    // `table::bless` refuses a changed one at the blessed version, so a new hash
    // would force a bump of every converter. This runs in tests, never at launch.
    hash::fnv1a64(&first)
}

fn convert(converter: &Converter, source: &Path, name: &str, run: &str) -> Vec<u8> {
    let dir = rusty_test_support::temp::dir().join(format!("{name}.{run}"));
    std::fs::create_dir_all(&dir).expect("a conversion folder");
    // Outside the cache (no stamp, no tagged folder), `out` beside the fixture: an
    // on-disk texture's URI is then the same on every machine.
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

fn fixtures(extension: &str) -> Vec<PathBuf> {
    let dir = rusty_core::project::checkout().join("tests/fixtures/models");
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("the model fixtures")
        .map(|e| e.expect("a fixtures entry").path())
        .filter(|p| rusty_core::paths::extension(p) == extension)
        .collect();
    found.sort();
    found
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests;
