//! Tests for reading a recipe's image files (#1491).

use super::*;
use rusty_procgen::recipe::{ImageEncoding, ImageTexture, Node, OpKind};

fn image_recipe(path: &str) -> TextureRecipe {
    let op = OpKind::Image(ImageTexture {
        path: path.into(),
        encoding: ImageEncoding::Linear,
    });
    let node = Node {
        id: "photo".into(),
        op,
        inputs: vec![],
    };
    TextureRecipe::new(4, vec![node])
}

fn write_png(path: &Path) {
    image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]))
        .save(path)
        .unwrap();
}

#[test]
fn a_path_is_read_from_the_root() {
    let root = rusty_test_support::temp::dir().join("rusty_sources_root_1491");
    std::fs::create_dir_all(root.join("textures")).unwrap();
    write_png(&root.join("textures").join("red.png"));
    let sources = load_recipe(&image_recipe("textures/red.png"), &root)
        .unwrap()
        .sources;
    assert!(sources.get("textures/red.png").is_none(), "decoded lazily");
    let source = sources.load("textures/red.png").expect("found").unwrap();
    assert_eq!(source.texels()[0], [1.0, 0.0, 0.0, 1.0]);
    assert!(outside_root(&root.join("textures").join("red.png"), &root).is_none());
}

#[test]
fn a_missing_file_is_an_error_naming_the_node_and_the_path() {
    let root = rusty_test_support::temp::dir().join("rusty_sources_missing_1491");
    std::fs::create_dir_all(&root).unwrap();
    let err = load_recipe(&image_recipe("textures/nope.png"), &root).unwrap_err();
    assert!(
        err.contains("'photo'") && err.contains("textures/nope.png"),
        "{err}"
    );
}

#[test]
fn a_file_that_does_not_decode_fails_the_bake_naming_the_node() {
    let root = rusty_test_support::temp::dir().join("rusty_sources_corrupt_2002");
    std::fs::create_dir_all(root.join("textures")).unwrap();
    std::fs::write(root.join("textures").join("cut.png"), b"\x89PNG trunc").unwrap();
    let loaded = load_recipe(&image_recipe("textures/cut.png"), &root).unwrap();
    let err = rusty_procgen::evaluate_with(&loaded.recipe, &loaded.sources).unwrap_err();
    let err = err.to_string();
    assert!(
        err.contains("\"photo\"") && err.contains("textures/cut.png"),
        "{err}"
    );
}

#[test]
fn a_file_beside_the_root_is_outside_it() {
    let base = rusty_test_support::temp::dir().join("rusty_sources_outside_1491");
    let root = base.join("project");
    std::fs::create_dir_all(&root).unwrap();
    let file = base.join("shared.png");
    write_png(&file);
    assert!(outside_root(&file, &root).is_some());
    // Read all the same: outside is allowed, it only warns.
    assert!(load_recipe(&image_recipe("../shared.png"), &root).is_ok());
}

#[test]
fn a_broken_recipe_is_reported_before_any_file_is_read() {
    let mut recipe = image_recipe("textures/no_such_1491.png");
    recipe.nodes.push(Node {
        id: "inv".into(),
        op: OpKind::Invert,
        inputs: vec!["nowhere".into()],
    });
    let err = load_recipe(&recipe, Path::new("")).unwrap_err();
    assert!(err.contains("unknown input \"nowhere\""), "{err}");
}
