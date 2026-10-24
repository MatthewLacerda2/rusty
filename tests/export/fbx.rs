//! An `.fbx` project exported, then played from the export.

use std::path::{Path, PathBuf};

use super::{
    cache, entity, export, play, play_scenario, project, tree, with_extension, CACHE, SCENARIO,
};

/// The fixture quad, its `Quad` textured by the embedded `Red`.
pub(super) const QUAD: &str = "embedded_textures.fbx";
pub(super) const SOURCE: &str = "assets/models/embedded_textures.fbx";

/// Instantiate the quad by its source path, which the export leaves behind.
pub(super) const START: &str =
    r#"Scene.Instantiate("assets/models/embedded_textures.fbx::Quad", "ExportedQuad")"#;

#[test]
fn an_export_carries_the_cache_and_leaves_the_fbx_behind() {
    let game = project(&[QUAD], START);
    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());
    let files = tree(&out);
    assert_eq!(
        with_extension(&files, &["fbx", "blend"]),
        Vec::<&str>::new()
    );
    let glb = format!("{CACHE}/{SOURCE}.glb");
    assert!(files.contains_key(&glb), "{:?}", files.keys());
    assert!(files.contains_key("assets/scripts/spawn.lua"));
    assert!(files.contains_key("project.rusty"));
    assert!(
        game.join(rusty::core::paths::native(SOURCE)).is_file(),
        "the project keeps its source"
    );

    let again = crate::temp::dir().join("again");
    assert!(export(&game, &again).status.success());
    assert!(
        tree(&again) == files,
        "two exports of one project are byte-identical"
    );
}

#[test]
fn a_source_that_fails_to_convert_fails_the_export_naming_it() {
    let game = project(&[QUAD], START);
    std::fs::write(
        game.join("assets").join("models").join("broken.fbx"),
        "not an fbx",
    )
    .unwrap();
    let out = crate::temp::dir().join("out");
    let result = export(&game, &out);
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("broken.fbx"), "{stderr}");
    assert!(tree(&out).is_empty(), "nothing is written");
}

#[test]
fn an_output_holding_files_is_refused() {
    let game = project(&[QUAD], START);
    let out = crate::temp::dir().join("out");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("keep.txt"), "mine").unwrap();
    assert_eq!(export(&game, &out).status.code(), Some(1));
    assert_eq!(tree(&out).len(), 1, "left as it was");
}

#[test]
fn the_export_plays_from_its_cache_and_converts_nothing() {
    let game = project(&[QUAD], START);
    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());
    let before = tree(&out);
    let (ran, results) = play(&out, &[]);
    assert!(
        ran.status.success() && results["passed"] == true,
        "the scenario passes"
    );
    let quad = entity(&results, "ExportedQuad");
    assert!(quad["bounds"].is_object(), "the quad has geometry: {quad}");
    let reference = format!("{SOURCE}#tex/Red");
    assert_eq!(quad["material"]["texture"], reference.as_str());
    let after = tree(&out);
    assert!(
        cache(&after) == cache(&before),
        "no conversion touched the cache"
    );
    assert_eq!(
        with_extension(&after, &["fbx", "blend"]),
        Vec::<&str>::new()
    );
    let red = extracted(
        &out.join(rusty::core::paths::native(&format!("{CACHE}/{SOURCE}.glb"))),
        "Red",
    );
    assert!(red.is_file(), "{} is exported", red.display());
}

/// What [`START`] spawns, scaled to a metre (it is 1 cm, the FBX default unit),
/// with the camera half a metre in front of its face.
const FRAME: &str = r#"Transform.SetLocalScale(quad, 100, 100, 100)
Camera.SetPosition(0.5, 0.5, 1.2)
Camera.SetYaw(-90)
Camera.SetPitch(0)"#;

#[test]
fn gpu_the_exported_quad_renders_red_from_its_cache() {
    // Coloured only by its texture, the quad rendered black before the converter
    // read a colour the file leaves out as white (#1280).
    let game = project(&[QUAD], &format!("local quad = {START}\n{FRAME}"));
    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());
    let png = crate::temp::dir().join("quad.png");
    let shot = format!(
        "{SCENARIO}Harness.Screenshot({:?})\n",
        png.to_string_lossy()
    );
    let (ran, results) = play_scenario(&out, &shot, &[]);
    assert!(
        ran.status.success() && results["passed"] == true,
        "the scenario passes: {results}"
    );
    if !png.is_file() {
        // Only a real skip may pass (#885).
        assert!(!rusty::render::gpu_required(), "no shot: {results}");
        eprintln!("no GPU adapter — skipping the pixels");
        return;
    }
    let image = image::open(&png).unwrap().to_rgb8();
    let red = image
        .pixels()
        .filter(|p| p[0] > 96 && p[1] < 32 && p[2] < 32);
    let share = red.count() as f32 / image.pixels().len() as f32;
    assert!(share > 0.25, "{share:.3} of the frame is the red quad");
}

/// The file the cached `.glb` at `glb` names for the texture `identity`: where
/// `texture_ref` resolves `<source>#tex/<identity>`, the source present or not.
pub(super) fn extracted(glb: &Path, identity: &str) -> PathBuf {
    let uris = rusty::asset::convert::glb::image_uris(glb).expect("the cache reads");
    let (_, uri) = uris
        .into_iter()
        .find(|(name, _)| name == identity)
        .expect(identity);
    glb.parent().unwrap().join(uri)
}
