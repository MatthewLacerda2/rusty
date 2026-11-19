//! The cooking code's output can't change without a bump of [`COOKER_VERSION`]
//! (#2332), as a converter's can't without its version (`convert::fingerprint`).
//!
//! A stale cook keeps every cooked file made before it current, and a stale
//! unwrap misplaces every baked lightmap, so a test holds the bump: the fixture
//! model is cooked and its cooked file hashed, and [`BLESSED`] pins that hash at
//! the version it was made by. The hash is Linux x86_64's (CI's suite): another
//! platform's maths library may round a float differently.

use super::stamp::COOKER_VERSION;

/// The version and the cooked fixture's hash at it. Re-blessed only after a bump:
/// the failure message prints the new row.
const BLESSED: (u32, &str) = (1, "38a0fd8f354cb27d");

/// Whether `now` (the fixture's hash at `version`) holds `blessed`, or what to do.
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
    let path = super::fixture::model("version");
    // The file itself: its stamp names the fixture's bytes and settings, both fixed.
    super::with_writes(true, || crate::import_file(&path).unwrap());
    let bytes = std::fs::read(super::cooked_path(&path).unwrap()).unwrap();
    let now = format!("{:016x}", rusty_core::hash::content(&bytes));
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
