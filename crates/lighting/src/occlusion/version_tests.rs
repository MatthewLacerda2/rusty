//! [`OCCLUSION_VERSION`]'s rule held (#2348): the bake's bytes for one small fixed
//! scene are pinned beside the version, so a change to what the bake writes for the
//! same objects fails until the version is bumped, which makes Generate Lighting
//! re-bake a PVS an older engine made instead of keeping it.
//!
//! Linux only: a bake is byte-identical on one machine, not across architectures
//! (`docs/testing.md`, *Rules*), and the pin is Linux's, as the converters' are.

use crate::occlusion::test_level::corridors;
use crate::occlusion::{bake_occlusion, OCCLUSION_VERSION};
use rusty_scene::lighting::occlusion::encode;

/// The version the pin was taken at, and the content hash of the corridors level's
/// PVS with its stamp's version zeroed (so the hash is the bake's bytes alone).
/// Under its one test's `cfg`, so no other build holds an unused constant.
#[cfg(target_os = "linux")]
const PINNED: (u32, u64) = (1, 0x69fc_da1c_7ac0_6094);

/// The corridors level's PVS bytes, the version field zeroed.
fn bytes_hash() -> u64 {
    let mut data = bake_occlusion(&corridors(), 2).0.expect("occluders");
    data.stamp.version = 0;
    rusty_core::hash::content(&encode(&data))
}

#[test]
#[cfg(target_os = "linux")]
fn the_occlusion_version_is_bumped_when_the_bake_writes_other_bytes() {
    let hash = bytes_hash();
    let (version, pinned) = PINNED;
    if OCCLUSION_VERSION != version {
        panic!(
            "OCCLUSION_VERSION is {OCCLUSION_VERSION}, pinned at {version}: re-pin \
             `PINNED` to ({OCCLUSION_VERSION}, {hash:#x})"
        );
    }
    assert_eq!(
        hash, pinned,
        "the occlusion bake writes other bytes for the same objects: bump \
         `OCCLUSION_VERSION` and re-pin `PINNED` to (the new version, {hash:#x}), \
         or `test_level::corridors()` or the fingerprint's encoding changed: then \
         re-pin without a bump"
    );
}

#[test]
fn the_bake_writes_the_same_bytes_twice() {
    assert_eq!(
        bytes_hash(),
        bytes_hash(),
        "the pin needs a repeatable bake"
    );
}
