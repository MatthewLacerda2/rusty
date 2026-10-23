//! The resolved-reference cache (#1256): one read per source until it converts again.

use super::resolved::reads;
use super::tests::{base_color_map, quad, quad_at};
use super::{file, reference};
use crate::core::fs::freshness::MISS_RETRY_FRAMES;

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
