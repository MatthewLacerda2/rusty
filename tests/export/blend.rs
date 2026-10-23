//! A `.blend` project exported through Blender, then played with no Blender at all.
//! The export needs Blender 5+, so this is skipped by default and run by CI's
//! `blender` job (`--run-ignored only`), like `asset::convert::blend`'s.

use super::{cache, entity, export, play, project, tree, with_extension, CACHE};

const START: &str = r#"Scene.Instantiate("assets/models/textured.blend::Cube", "ExportedCube")"#;

#[test]
// lint: allow(skip) needs Blender 5+; CI's `blender` job installs it and runs this
#[ignore]
fn an_exported_blend_plays_without_starting_blender() {
    let game = project(&["textured.blend", "textured_bricks.png"], START);
    let out = crate::temp::dir().join("out");
    assert!(export(&game, &out).status.success());
    let files = tree(&out);
    assert_eq!(
        with_extension(&files, &["fbx", "blend"]),
        Vec::<&str>::new()
    );
    let glb = format!("{CACHE}/assets/models/textured.blend.glb");
    assert!(files.contains_key(&glb), "{:?}", files.keys());

    // A "Blender" that only leaves a mark: the player must never start it.
    let dir = crate::temp::dir();
    let (fake, mark) = (dir.join("blender.sh"), dir.join("started"));
    let script = format!("#!/bin/sh\ntouch {:?}\nexit 1\n", mark.to_string_lossy());
    std::fs::write(&fake, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (ran, results) = play(&out, &[("RUSTY_BLENDER", fake.as_path())]);
    assert!(
        ran.status.success() && results["passed"] == true,
        "the scenario passes"
    );
    let cube = entity(&results, "ExportedCube");
    assert!(cube["bounds"].is_object(), "the cube has geometry: {cube}");
    assert!(!mark.exists(), "the player started Blender");
    assert!(
        cache(&tree(&out)) == cache(&files),
        "no conversion touched the cache"
    );
}
