//! An `.fbx` project exported, then played from the export.

use std::path::{Path, PathBuf};

use super::{cache, entity, export, play, project, tree, with_extension, CACHE};

/// The fixture quad, its `Quad` textured by the embedded `Red`.
const QUAD: &str = "embedded_textures.fbx";
const SOURCE: &str = "assets/models/embedded_textures.fbx";

/// Instantiate the quad by its source path, which the export leaves behind.
const START: &str =
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
    assert!(game.join(SOURCE).is_file(), "the project keeps its source");

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
    std::fs::write(game.join("assets/models/broken.fbx"), "not an fbx").unwrap();
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

/// The file the cached `.glb` at `glb` names for the texture `identity`: where
/// `texture_ref` resolves `<source>#tex/<identity>`, the source present or not.
fn extracted(glb: &Path, identity: &str) -> PathBuf {
    let uris = rusty::asset::convert::glb::image_uris(glb).expect("the cache reads");
    let (_, uri) = uris
        .into_iter()
        .find(|(name, _)| name == identity)
        .expect(identity);
    glb.parent().unwrap().join(uri)
}

#[test]
fn gpu_the_export_plays_from_its_cache_and_converts_nothing() {
    let png = crate::temp::dir().join("quad.png");
    // A failed preview names an entity after its error, for the snapshot to show.
    let preview = format!(
        "local ok, err = pcall(Debug.Preview, {SOURCE:?}, {:?}, {{ resolution = 64 }})\n\
         if not ok then Scene.CreateEntity(\"PreviewFailed: \" .. tostring(err)) end",
        png.to_string_lossy()
    );
    let game = project(&[QUAD], &format!("{START}\n{preview}"));
    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());
    let before = tree(&out);
    let (ran, results) = play(&out, &[]);
    assert!(
        ran.status.success() && results["passed"] == true,
        "the scenario passes"
    );

    let names = results["final_snapshot"]["entities"].as_array().unwrap();
    let failed = names.iter().find(|e| {
        e["name"]
            .as_str()
            .unwrap_or("")
            .starts_with("PreviewFailed")
    });
    assert!(failed.is_none(), "{failed:?}");
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
    let red = extracted(&out.join(format!("{CACHE}/{SOURCE}.glb")), "Red");
    assert!(red.is_file(), "{} is exported", red.display());
    // The preview renders from the cache; with no adapter it writes nothing.
    if png.is_file() {
        assert!(image::open(&png).is_ok(), "the preview is a PNG");
    }
}
