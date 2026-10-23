//! A model whose texture lies outside `assets/` fails the export, naming it.

use super::{export, fixture, project, tree};

#[test]
fn a_texture_outside_assets_fails_the_export_naming_it() {
    let game = project(&[], "");
    let textures = game.join("textures");
    std::fs::create_dir_all(&textures).unwrap();
    let png = textures.join("gone.png");
    std::fs::copy(fixture("textured_bricks.png"), &png).unwrap();
    // The fixture triangle, its texture pointed at `<game>/textures/gone.png`.
    let fbx = std::fs::read_to_string(fixture("missing_texture.fbx")).unwrap();
    let at = png.to_string_lossy().replace('\\', "/");
    let fbx = fbx
        .replace("\"/missing/gone.png\"", &format!("{at:?}"))
        .replace(
            "RelativeFilename: \"gone.png\"",
            "RelativeFilename: \"../../textures/gone.png\"",
        );
    std::fs::write(game.join("assets/models/tri.fbx"), fbx).unwrap();

    let out = crate::temp::dir().join("out");
    let result = export(&game, &out);
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("assets/models/tri.fbx: textures/gone.png"),
        "{stderr}"
    );
    assert!(tree(&out).is_empty(), "the output is left empty");
}
