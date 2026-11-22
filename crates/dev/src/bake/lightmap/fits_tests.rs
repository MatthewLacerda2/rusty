use std::cell::RefCell;

use glam::Vec2;

use super::fits::{check, covered, estimate, result_bytes, Estimate};
use crate::memory::Machine;
use rusty_lighting::lightmap::{BakeMesh, BakeScene};
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};
use rusty_script::scripting::ConsoleLogs;

const GB: u64 = 1 << 30;

fn machine(available: u64, total: u64) -> Option<Machine> {
    Some(Machine { available, total })
}

fn held(maps: u64, pages: u64) -> Estimate {
    Estimate {
        maps,
        pages,
        ..Default::default()
    }
}

#[test]
fn a_bake_past_the_machine_is_refused_naming_what_holds_it_and_what_to_lower() {
    let s = BakeSettings::default();
    let refused = check(held(18 * GB, 2 * GB), &s, machine(10 * GB, 16 * GB)).unwrap_err();
    assert!(refused.contains("about 20.0 GB"), "{refused}");
    assert!(
        refused.contains("finished maps 18.0 GB") && refused.contains("pages 2.0 GB"),
        "{refused}"
    );
    assert!(
        refused.contains("16.0 GB this machine has in all"),
        "{refused}"
    );
    assert!(refused.contains("texelsPerUnit") && refused.contains("maxLightmapSize"));
}

#[test]
fn a_bake_past_what_is_free_warns_and_runs() {
    rusty_logging::init_for_tests();
    let console = RefCell::new(ConsoleLogs::new());
    rusty_logging::forward(&console);
    let s = BakeSettings::default();
    assert_eq!(
        check(held(12 * GB, 0), &s, machine(10 * GB, 16 * GB)),
        Ok(())
    );
    let said = rusty_logging::forward(&console);
    assert!(
        said.iter().any(|l| l.contains("10.0 GB it has free")),
        "{said:?}"
    );
    assert_eq!(check(held(GB, 0), &s, machine(10 * GB, 16 * GB)), Ok(()));
    assert_eq!(
        check(held(99 * GB, 0), &s, None),
        Ok(()),
        "an unread machine holds nothing back"
    );
    let quiet = rusty_logging::forward(&console);
    assert!(
        !quiet.iter().any(|l| l.contains("LightmapBake")),
        "{quiet:?}"
    );
}

#[test]
fn the_peak_holds_the_kept_maps_and_the_larger_of_the_trace_and_the_pages() {
    let e = Estimate {
        maps: 5,
        in_flight: 7,
        oidn: 11,
        pages: 3,
        progressive: 2,
    };
    assert_eq!(e.peak(), 5 + 2 + 11);
    assert_eq!(Estimate { pages: 20, ..e }.peak(), 5 + 2 + 20);
}

#[test]
fn the_estimate_grows_by_the_maps_covered_texels_alone() {
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
    let extra = 6 * texels * result_bytes(&s, false);
    assert_eq!(four.peak() - one.peak(), extra, "{one} / {four}");
    let progressive = estimate(&scene(2), &s, true, 2);
    assert!(
        progressive.peak() > one.peak(),
        "a progressive bake keeps every texel's state"
    );
    let oidn = BakeSettings {
        denoiser: Denoiser::Oidn,
        ..s
    };
    assert!(
        estimate(&scene(2), &oidn, false, 2).peak() > one.peak(),
        "OIDN adds its charts"
    );
    assert_eq!(
        estimate(&BakeScene::default(), &s, false, 8),
        Estimate::default(),
        "nothing to bake"
    );
}

#[test]
fn a_map_whose_uvs_cover_a_sliver_of_its_square_counts_the_sliver() {
    let mesh = BakeMesh {
        lightmap_uvs: vec![Vec2::ZERO, Vec2::new(0.1, 0.0), Vec2::new(0.0, 1.0)],
        indices: vec![0, 1, 2],
        ..Default::default()
    };
    let (edge, square) = (1000, 1000 * 1000);
    let sliver = covered(&mesh, edge);
    assert!(sliver >= square / 20 && sliver < square / 10, "{sliver}");
    assert_eq!(covered(&mesh, 1), 1, "never past the square");
}

#[test]
fn charts_past_the_gate_count_one_assembly_and_smaller_ones_each_their_own() {
    let flying = |texels_per_unit, threads| {
        let s = BakeSettings {
            texels_per_unit,
            ..Default::default()
        };
        let scene = crate::bake::lightmap::memory_tests::planes(2, 20.0);
        estimate(&scene, &s, false, threads).in_flight
    };
    // 500² squares lie under `ASSEMBLE_ALONE` (512²), so two assemble at once; 1000² squares
    // lie past it, so the second only traces beside the first's assembly.
    assert_eq!(flying(25.0, 2), 2 * flying(25.0, 1));
    let (one, two) = (flying(50.0, 1), flying(50.0, 2));
    assert!(two > one && two - one < one / 2, "{one} / {two}");
}
