//! The warning for textures a bench drew uncompressed for want of an import (#2319).

use super::uncompressed;

#[test]
fn textures_drawn_uncompressed_for_want_of_an_import_are_warned_of_with_their_size() {
    assert_eq!(uncompressed(rusty_render::Uploads::default()), None);
    let one = rusty_render::Uploads {
        count: 1,
        bytes: 341 * 4,
    };
    let w = uncompressed(one).unwrap();
    assert!(
        w.starts_with("uploaded uncompressed: 1 texture, 0.0 MB"),
        "{w}"
    );
    let many = rusty_render::Uploads {
        count: 3,
        bytes: 3 << 20,
    };
    let w = uncompressed(many).unwrap();
    assert!(w.contains("3 textures, 3.0 MB"), "{w}");
    assert!(w.contains("make import"), "{w}");
}
