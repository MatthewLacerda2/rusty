//! An FBX's on-disk texture is a file its conversion depends on (#1189): adding
//! it beside the FBX after a conversion that skipped it re-converts.

use super::super::super::convert_to;
use super::fixture;
use std::path::Path;

/// The stamp file of the cache at `out`.
fn record(out: &Path) -> String {
    std::fs::read_to_string(crate::asset::convert::record::stamp_path(out)).unwrap()
}

/// Whether the stamp file of the cache at `out` records a skip.
fn skips(out: &Path) -> bool {
    record(out).lines().any(|l| l.starts_with("skipped "))
}

#[test]
fn a_missing_texture_added_later_reconverts_and_its_skip_is_gone() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("quad.fbx"), dir.join("quad.fbx.glb"));
    std::fs::copy(fixture("missing_texture.fbx"), &source).unwrap();
    convert_to(&source, &out).unwrap();
    assert!(skips(&out), "gone.png is missing");

    // The FBX names it `gone.png`, relative to itself.
    let texture = dir.join("gone.png");
    std::fs::write(&texture, b"\x89PNG").unwrap();
    convert_to(&source, &out).unwrap();
    assert!(!skips(&out), "re-converted with gone.png");
    let found = record(&out);

    std::fs::write(&texture, b"\x89PNG changed").unwrap();
    convert_to(&source, &out).unwrap();
    assert_ne!(record(&out), found, "re-stamped with the new bytes");

    std::fs::remove_file(&texture).unwrap();
    convert_to(&source, &out).unwrap();
    assert!(skips(&out), "re-converted without gone.png");
}
