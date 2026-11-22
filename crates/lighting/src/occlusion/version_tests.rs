use crate::occlusion::test_level::corridors;
use crate::occlusion::{bake_occlusion, OCCLUSION_VERSION};
use rusty_scene::lighting::occlusion::encode;

#[cfg(target_os = "linux")]
const PINNED: (u32, u64) = (2, 0x6d14_53fc_688c_888a);

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
