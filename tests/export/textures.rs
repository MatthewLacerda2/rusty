//! A model whose texture lies outside `assets/` fails the export, naming it; an
//! image's compressed copy ships, and the player never re-makes it; an image that
//! does not decode ships uncompressed, never failing the export (#1574).

use super::{export, fixture, play, project, tree, CACHE};

#[test]
fn an_images_compressed_copy_ships_and_the_player_writes_none() {
    let game = project(&[], "");
    let textures = game.join("assets").join("textures");
    std::fs::create_dir_all(&textures).unwrap();
    std::fs::copy(fixture("textured_bricks.png"), textures.join("bricks.png")).unwrap();
    std::fs::write(textures.join("broken.png"), b"not a png").unwrap();
    let out = crate::temp::dir().join("out");
    let exported = export(&game, &out);
    assert!(exported.status.success(), "{exported:?}");
    let shipped = tree(&out);
    let cached = format!("{CACHE}/assets/textures/bricks.png.bctex");
    assert!(shipped.contains_key(&cached), "{:?}", shipped.keys());
    assert!(
        shipped.contains_key("assets/textures/broken.png"),
        "as it is"
    );
    let (ran, _) = play(&out, &[]);
    assert!(ran.status.success());
    // The player's first launch seeds the starter content (an image among it);
    // it compresses none of it, and leaves the shipped copy as it was.
    let after = tree(&out);
    assert_eq!(after.get(&cached), shipped.get(&cached));
    let made: Vec<_> = after.keys().filter(|k| k.ends_with(".bctex")).collect();
    assert_eq!(made, [&cached], "the player compressed nothing");
}

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
    std::fs::write(game.join("assets").join("models").join("tri.fbx"), fbx).unwrap();

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
