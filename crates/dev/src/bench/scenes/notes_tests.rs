//! The warnings for textures a bench drew uncompressed for want of an import
//! (#2319), and for models its loads cooked for want of one (#2332).

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

#[test]
fn a_scene_whose_loads_cooked_its_models_is_warned_of() {
    use super::super::load::Load;
    let load = |cooked: f64| Load {
        steps: Vec::new(),
        counts: vec![("models_cooked".to_string(), cooked)],
    };
    assert_eq!(super::cooked_at_load(&[load(0.0), load(0.0)]), None);
    assert_eq!(super::cooked_at_load(&[]), None);
    let w = super::cooked_at_load(&[load(0.0), load(3.0), load(1.0)]).unwrap();
    assert!(
        w.starts_with("its models were cooked at load (3 in a load)"),
        "{w}"
    );
    assert!(w.contains("make import"), "{w}");
}
