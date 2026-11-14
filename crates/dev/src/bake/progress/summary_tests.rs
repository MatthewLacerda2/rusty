//! The end of a bake's log (#1891) on a fake clock: what each step took, and the
//! slowest lightmap meshes from fake timings.

use std::time::Duration;

use super::summary::SLOWEST;
use super::tests::{lightmaps, logged};
use super::{MeshCost, Progress, Step};

fn pages(done: u64) -> Progress {
    Progress {
        step: Step::LightmapPages,
        done,
        total: 3,
        meshes: None,
    }
}

fn texts(run: impl FnOnce(&mut super::BakeLog, &dyn Fn(u64))) -> Vec<String> {
    logged(run).into_iter().map(|l| l.text).collect()
}

#[test]
fn each_step_takes_until_the_next_begins_and_they_add_up() {
    let lines = texts(|log, at| {
        at(0);
        log.observe(lightmaps(0, 0, (0, 0)));
        at(4);
        log.observe(lightmaps(0, 1000, (0, 2)));
        at(64);
        log.observe(pages(0));
        at(70);
        log.observe(pages(3));
        at(75);
        log.finish();
    });
    let end = &lines[lines.len() - 3..];
    assert_eq!(
        end,
        [
            "[Lighting] lightmaps took 1 min 4 s (1.0k texels)",
            "[Lighting] lightmap pages took 11 s (3 pages)",
            "[Lighting] done in 1 min 15 s",
        ]
    );
}

/// Every second has a line (#1891): the first step begins with the plan, and the work
/// between two steps is its own line, so a Generate-shaped run adds up.
#[test]
fn the_plan_and_the_work_between_steps_are_counted_and_add_up() {
    let probes = Progress {
        step: Step::LightProbes { bounce: 1, max: 5 },
        done: 0,
        total: 9,
        meshes: None,
    };
    let lines = texts(|log, at| {
        log.plan(&Step::GENERATE);
        at(3);
        log.observe(lightmaps(0, 1000, (0, 2)));
        at(10);
        log.observe(pages(0));
        at(12);
        log.enter(Step::WritingLightmaps);
        at(15);
        log.enter(Step::ProbePlacement);
        at(20);
        log.observe(probes);
        at(30);
        log.enter(Step::Saving);
        at(40);
        log.finish();
    });
    let end = &lines[lines.len() - 7..];
    assert_eq!(
        end,
        [
            "[Lighting] lightmaps took 10 s (1.0k texels)",
            "[Lighting] lightmap pages took 2 s (3 pages)",
            "[Lighting] writing lightmaps took 3 s",
            "[Lighting] probe placement took 5 s",
            "[Lighting] light probes (bounce 1) took 10 s (9 probes)",
            "[Lighting] saving took 10 s",
            "[Lighting] done in 40 s",
        ]
    );
}

#[test]
fn the_slowest_meshes_are_named_slowest_first_and_cut_at_fifteen() {
    let mesh = |i: u64| MeshCost {
        name: format!("m{i}"),
        texels: 1000,
        busy: Duration::from_millis(100 * i),
    };
    let lines = texts(|log, at| {
        at(0);
        log.observe(lightmaps(0, 20_000, (0, 20)));
        log.mesh_costs((0..20).map(mesh).collect());
        at(10);
        log.finish();
    });
    let named: Vec<&String> = lines
        .iter()
        .filter(|l| l.starts_with("[Lighting]   "))
        .collect();
    assert_eq!(named.len(), SLOWEST);
    assert_eq!(
        named[0],
        "[Lighting]   m19: 1.0k texels, 1.9 s, 526 texels/s"
    );
    assert_eq!(
        named[SLOWEST - 1],
        "[Lighting]   m5: 1.0k texels, 0.5 s, 2.0k texels/s"
    );
    let header = "[Lighting] slowest lightmap meshes (worker time over every thread):";
    assert!(lines.contains(&header.to_string()));
}

#[test]
fn a_mesh_that_took_no_time_has_no_rate_and_a_log_without_meshes_lists_none() {
    let free = MeshCost {
        name: "Quad".into(),
        texels: 4,
        busy: Duration::ZERO,
    };
    let lines = texts(|log, at| {
        at(0);
        log.observe(lightmaps(0, 4, (0, 1)));
        log.mesh_costs(vec![free]);
        log.finish();
    });
    assert!(lines.contains(&"[Lighting]   Quad: 4 texels, 0.0 s".to_string()));
    let bare = texts(|log, _| {
        log.observe(lightmaps(0, 4, (0, 1)));
        log.finish();
    });
    assert!(!bare.iter().any(|l| l.contains("slowest")), "{bare:?}");
}
