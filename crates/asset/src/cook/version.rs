//! The cooking code's output can't change without a bump of [`COOKER_VERSION`]:
//! fixture models are cooked and their cooked files hashed, and [`BLESSED`] pins that
//! hash. The hash is Linux x86_64's (CI's suite): another platform's maths library may
//! round a float differently.

use super::stamp::COOKER_VERSION;

const BLESSED: (u32, &str) = (3, "179dd9e0ef11c74d");

fn check(blessed: (u32, &str), version: u32, now: &str) -> Result<(), String> {
    match (blessed.0 == version, blessed.1 == now) {
        (true, true) => Ok(()),
        (true, false) => Err(format!(
            "the cooking code's output changed at COOKER_VERSION {version}: bump \
             COOKER_VERSION in crates/asset/src/cook/stamp.rs, then set BLESSED in \
             crates/asset/src/cook/version.rs to ({}, \"{now}\")",
            version + 1
        )),
        (false, _) => Err(format!(
            "COOKER_VERSION is {version} but BLESSED was made at {}: set BLESSED in \
             crates/asset/src/cook/version.rs to ({version}, \"{now}\")",
            blessed.0
        )),
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn the_cooked_output_holds_its_blessed_hash() {
    use rusty_test_support::models::{quad_file, skinned_buffer, SKINNED_GLTF, TRIANGLE_GLTF};
    let obj = super::fixture::model("version");
    let dir = obj.parent().unwrap().to_path_buf();
    let flat = dir.join("flat.gltf");
    std::fs::write(&flat, TRIANGLE_GLTF).unwrap();
    std::fs::write(dir.join("model.bin"), skinned_buffer()).unwrap();
    let skinned = dir.join("skinned.gltf");
    std::fs::write(&skinned, SKINNED_GLTF).unwrap();
    let quad = std::path::PathBuf::from(quad_file("rusty_cook_version_quad"));
    let mut hash = rusty_core::hash::Content::default();
    for model in [&obj, &flat, &skinned, &quad] {
        let file = super::fixture::cooked_file(model);
        let _ = std::fs::remove_file(&file);
        super::with_writes(true, || crate::import_file(model).unwrap());
        hash.write(&std::fs::read(&file).unwrap());
    }
    let now = format!("{:016x}", hash.finish());
    if let Err(e) = check(BLESSED, COOKER_VERSION, &now) {
        panic!("{e}");
    }
}

#[test]
fn an_unchanged_output_at_its_version_passes() {
    assert_eq!(check((3, "aa"), 3, "aa"), Ok(()));
}

#[test]
fn a_changed_output_at_the_same_version_asks_for_the_bump() {
    let e = check((3, "aa"), 3, "bb").unwrap_err();
    assert!(
        e.contains("output changed at COOKER_VERSION 3: bump"),
        "{e}"
    );
    assert!(e.contains("(4, \"bb\")"), "{e}");
}

#[test]
fn a_bumped_version_asks_for_the_re_bless() {
    for now in ["aa", "bb"] {
        let e = check((3, "aa"), 4, now).unwrap_err();
        assert!(e.contains("BLESSED was made at 3"), "{e}");
        assert!(e.contains(&format!("(4, \"{now}\")")), "{e}");
    }
}
