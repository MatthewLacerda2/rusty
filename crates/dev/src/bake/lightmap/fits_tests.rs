//! Whether a bake fits the machine (#2312): refused past all it has, warned past
//! what is free, silent otherwise; the estimate grows with what the bake keeps.

use super::fits::{check, estimate, result_bytes, PACK};
use crate::memory::Machine;
use rusty_lighting::lightmap::BakeScene;
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

const GB: u64 = 1 << 30;

fn machine(available: u64, total: u64) -> Option<Machine> {
    Some(Machine { available, total })
}

#[test]
fn a_bake_past_the_machine_is_refused_naming_what_to_lower() {
    let s = BakeSettings::default();
    let refused = check(20 * GB, &s, machine(10 * GB, 16 * GB)).unwrap_err();
    assert!(refused.contains("about 20.0 GB"), "{refused}");
    assert!(
        refused.contains("16.0 GB this machine has in all"),
        "{refused}"
    );
    assert!(refused.contains("texelsPerUnit") && refused.contains("maxLightmapSize"));
}

#[test]
fn a_bake_past_what_is_free_warns_and_runs() {
    rusty_test_support::logs::init();
    rusty_test_support::logs::take();
    let s = BakeSettings::default();
    assert_eq!(check(12 * GB, &s, machine(10 * GB, 16 * GB)), Ok(()));
    let said = rusty_test_support::logs::take();
    assert!(
        said.iter().any(|l| l.contains("10.0 GB it has free")),
        "{said:?}"
    );
    assert_eq!(check(GB, &s, machine(10 * GB, 16 * GB)), Ok(()));
    assert_eq!(
        check(99 * GB, &s, None),
        Ok(()),
        "an unread machine holds nothing back"
    );
    let quiet = rusty_test_support::logs::take();
    assert!(
        !quiet.iter().any(|l| l.contains("LightmapBake")),
        "{quiet:?}"
    );
}

#[test]
fn the_estimate_counts_the_results_the_work_in_flight_and_the_progressive_state() {
    let scene = |n| crate::bake::lightmap::memory_tests::planes(n, 20.0);
    let s = BakeSettings {
        texels_per_unit: 25.0,
        denoiser: Denoiser::Filter,
        directional: true,
        ..Default::default()
    };
    let texels = 500 * 500;
    let (one, four) = (
        estimate(&scene(2), &s, false, 2),
        estimate(&scene(8), &s, false, 2),
    );
    let (extra, grew) = (6 * texels * result_bytes(&s, false), four - one);
    let by_results = grew >= extra && grew <= (PACK + 1) * extra;
    assert!(by_results, "four times grows by results, packed: {grew}");
    let progressive = estimate(&scene(2), &s, true, 2);
    assert!(
        progressive > one,
        "a progressive bake keeps every texel's state"
    );
    let oidn = BakeSettings {
        denoiser: Denoiser::Oidn,
        ..s
    };
    assert!(
        estimate(&scene(2), &oidn, false, 2) > one,
        "OIDN adds its charts"
    );
    assert_eq!(
        estimate(&BakeScene::default(), &s, false, 8),
        0,
        "nothing to bake"
    );
}
