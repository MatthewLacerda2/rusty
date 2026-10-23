//! The resolved-reference cache (#1256): one read per source until it converts
//! again, absolute keys across a project switch, and the callers' retry beat (#1269).

use std::path::PathBuf;

use super::resolved::{file_in, reads};
use super::tests::{base_color_map, quad, quad_at};
use super::{exists, file, reference};
use crate::asset::convert::cache_location;
use crate::core::fs::freshness::{Freshness, MISS_RETRY_FRAMES};

#[test]
fn a_source_is_read_once_until_it_converts_again() {
    let path = quad(|text| text);
    let red = base_color_map(&path, "Red");
    let blue = base_color_map(&path, "Blue");
    let before = reads();
    let pixels = std::fs::read(file(&red).expect("resolves")).unwrap();
    assert!(file(&red).is_some() && file(&blue).is_some());
    assert_eq!(
        reads(),
        before + 1,
        "one read serves every texture of the source"
    );

    // A re-conversion notes the source: the next resolution reads the new table.
    quad_at(&path, |text| {
        text.replace("\"Texture::Blue\"", "\"Texture::Red\"")
    });
    assert_eq!(base_color_map(&path, "Red"), red);
    assert_eq!(std::fs::read(file(&red).unwrap()).unwrap(), pixels);
    assert_eq!(reads(), before + 2);
}

#[test]
fn a_reference_with_no_cache_is_read_once_until_its_source_converts() {
    let path = quad(|text| text);
    let red = reference(&path.to_string_lossy(), "Red");
    let before = reads();
    for _ in 0..=2 * MISS_RETRY_FRAMES {
        assert_eq!(file(&red), None, "nothing converted yet");
    }
    assert_eq!(
        reads(),
        before + 1,
        "a cache that never appears is not re-read"
    );

    assert_eq!(base_color_map(&path, "Red"), red);
    assert!(
        file(&red).is_some_and(|f| f.exists()),
        "the conversion is picked up"
    );
}

#[test]
fn a_switch_to_another_project_resolves_its_namesake() {
    let (moved_to_a, moved_to_b) = (u64::MAX - 1, u64::MAX - 2);
    let root = crate::test_temp::dir();
    let (a, b) = (root.join("a"), root.join("b"));
    for dir in [&a, &b] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let quad_a = quad_at(&a.join("quad.fbx"), |text| text);
    // B's "Red" is the texture A calls "Blue".
    let quad_b = quad_at(&b.join("quad.fbx"), |text| {
        text.replace("\"Texture::Red\"", "\"Texture::Tmp\"")
            .replace("\"Texture::Blue\"", "\"Texture::Red\"")
            .replace("\"Texture::Tmp\"", "\"Texture::Blue\"")
    });
    // Both caches current: the switch itself writes nothing.
    base_color_map(&quad_a, "Red");
    base_color_map(&quad_b, "Red");
    let pixels = |f: Option<PathBuf>| std::fs::read(f.expect("resolves")).unwrap();
    let a_blue = pixels(file_in(moved_to_a, &a, "quad.fbx", "Blue"));
    assert_ne!(pixels(file_in(moved_to_a, &a, "quad.fbx", "Red")), a_blue);

    let before = reads();
    let b_red = pixels(file_in(moved_to_b, &b, "quad.fbx", "Red"));
    assert_eq!(b_red, a_blue, "the new project's own texture");
    assert!(file_in(moved_to_b, &b, "./quad.fbx", "Red").is_some_and(|f| f.is_absolute()));
    assert_eq!(reads(), before + 1, "one table, under one absolute key");
}

#[test]
fn a_glb_another_process_writes_is_picked_up_on_the_callers_beat() {
    let path = quad(|text| text);
    let red = base_color_map(&path, "Red");
    let glb = cache_location(&path).unwrap();
    let aside = glb.with_extension("aside");
    // lint: allow(atomic-write) moves the cache aside, as if never converted
    std::fs::rename(&glb, &aside).unwrap();
    assert_eq!(file(&red), None, "no cache to read");

    // The caller remembers its miss, and the `.glb` comes back outside the write log.
    let mut caller = Freshness::new();
    caller.miss(&red);
    // lint: allow(atomic-write) the test plays another process writing the cache
    std::fs::rename(&aside, &glb).unwrap();
    let picked_up = (1..=MISS_RETRY_FRAMES).find(|_| caller.poll_with(exists).stale.contains(&red));
    assert!(picked_up.is_some(), "within one beat of the caller");
    assert!(file(&red).is_some_and(|f| f.exists()));
}

#[test]
fn an_unparseable_glb_is_read_once_until_it_changes() {
    let path = quad(|text| text);
    let red = reference(&path.to_string_lossy(), "Red");
    let glb = cache_location(&path).unwrap();
    std::fs::create_dir_all(glb.parent().unwrap()).unwrap();
    // lint: allow(atomic-write) the test plays a truncated cache
    std::fs::write(&glb, b"glTF").unwrap();
    let before = reads();
    for _ in 0..=2 * MISS_RETRY_FRAMES {
        assert_eq!(file(&red), None, "nothing to resolve in a broken cache");
    }
    assert_eq!(reads(), before + 1, "the broken `.glb` is not re-parsed");

    // lint: allow(atomic-write) the test plays another process still writing it
    std::fs::write(&glb, b"glTF, longer").unwrap();
    assert_eq!(file(&red), None);
    assert_eq!(reads(), before + 2, "a changed `.glb` is read again");
}
