//! The staged cubes' commit (#1737): a cube that cannot be moved into place leaves
//! the probe set untouched, and a retry resumes where it failed.

use glam::Vec3;

use super::staged::StagedCubemaps;
use rusty_render::ibl::reflection_bake::BakedCube;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::ReflectionProbeSet;

#[test]
fn a_failed_move_touches_no_probe_and_a_retry_resumes() {
    let dir = rusty_test_support::temp::dir().join("rusty_1737_staged_retry");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut staged = StagedCubemaps::default();
    let cube = |bytes: &[u8]| BakedCube {
        bytes: bytes.to_vec(),
        irradiance: Sh9::zero(),
    };
    staged.write(&dir, &cube(b"cube 0")).unwrap();
    staged.write(&dir, &cube(b"cube 1")).unwrap();
    // A non-empty directory where cube 1 goes: its rename fails.
    let blocked = dir.join("reflection_probe_1.ktx2");
    std::fs::create_dir_all(blocked.join("x")).unwrap();
    let mut set = ReflectionProbeSet::default();
    for _ in 0..2 {
        set.add(Vec3::ZERO, Vec3::ONE);
    }
    let before = set.clone();
    assert!(staged.commit(&mut set).is_err(), "cube 1 is blocked");
    assert_eq!(set.probes, before.probes, "no probe points at a cube yet");
    std::fs::remove_dir_all(&blocked).unwrap();
    staged.commit(&mut set).unwrap();
    for (i, p) in set.probes.iter().enumerate() {
        let want = dir.join(format!("reflection_probe_{i}.ktx2"));
        assert_eq!(p.cubemap_path, want.to_string_lossy());
        assert_eq!(
            p.irradiance,
            Some(Sh9::zero()),
            "stored with its cube (#1697)"
        );
        assert_eq!(
            std::fs::read(&want).unwrap(),
            format!("cube {i}").into_bytes()
        );
    }
}
