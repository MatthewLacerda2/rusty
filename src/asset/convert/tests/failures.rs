//! Every failure of a converter's own I/O leaves the cache unstamped (#1168,
//! #1172): the FBX converter, failed through `stage::hook` at each step.

use super::super::stage::hook::{self, Step};
use super::super::{converter_for, refresh, Converter};
use std::path::{Path, PathBuf};

const STEPS: [Step; 3] = [Step::ReadSource, Step::ExtractTexture, Step::WriteGltf];

/// The fixture with embedded textures, so every step runs.
fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/embedded_textures.fbx")
}

/// `refresh` with the converter's I/O failing at `step`.
fn refresh_failing(converter: &Converter, step: Step, out: &Path) -> bool {
    hook::fail_at(Some(step));
    let result = refresh(converter, &fixture(), out);
    hook::fail_at(None);
    result.is_err()
}

/// Every name in `dir`, sorted, and the bytes of the cache and its stamp.
fn snapshot(dir: &Path, out: &Path) -> (Vec<String>, Vec<u8>, String) {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let stamp = std::fs::read_to_string(dir.join("quad.fbx.glb.source")).unwrap();
    (names, std::fs::read(out).unwrap(), stamp)
}

#[test]
fn a_failed_first_conversion_writes_no_cache() {
    let fbx = converter_for(&fixture()).expect("an FBX");
    for step in STEPS {
        let dir = crate::test_temp::dir().join(format!("{step:?}"));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("quad.fbx.glb");
        assert!(refresh_failing(fbx, step, &out), "{step:?} fails");
        assert!(!out.exists(), "{step:?}: no .glb");
        assert!(
            !dir.join("quad.fbx.glb.source").exists(),
            "{step:?}: no stamp"
        );
    }
}

#[test]
fn a_failed_reconversion_keeps_the_previous_cache_and_retries_next_time() {
    let fbx = converter_for(&fixture()).expect("an FBX");
    let newer = Converter {
        version: fbx.version + 1,
        ..*fbx
    };
    for step in STEPS {
        let dir = crate::test_temp::dir().join(format!("{step:?}"));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("quad.fbx.glb");
        refresh(fbx, &fixture(), &out).unwrap();
        let before = snapshot(&dir, &out);

        assert!(refresh_failing(&newer, step, &out), "{step:?} fails");
        assert_eq!(
            snapshot(&dir, &out),
            before,
            "{step:?}: the old cache, intact"
        );

        refresh(&newer, &fixture(), &out).unwrap();
        assert_ne!(
            snapshot(&dir, &out).2,
            before.2,
            "{step:?}: converted again"
        );
    }
}
