//! The step plan (#1897): said once as the bake begins, and the steps still to come
//! on every line after.

use super::tests::{lightmaps, logged};
use super::{BakeLog, Progress, Step};

fn probes(bounce: u32) -> Progress {
    Progress {
        step: Step::LightProbes { bounce, max: 5 },
        done: 0,
        total: 10,
        meshes: None,
    }
}

#[test]
fn the_plan_is_said_once_and_each_line_names_what_is_left() {
    let lines = logged(|log, at| {
        at(0);
        log.plan(&Step::GENERATE);
        log.observe(lightmaps(0, 0, (0, 0)));
        at(10);
        log.observe(probes(1));
        log.observe(probes(2));
        log.observe(Progress {
            step: Step::Occlusion,
            done: 0,
            total: 9,
            meshes: None,
        });
    });
    let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "[Lighting] steps: lightmaps → lightmap pages → light probes → reflection probes → occlusion",
            "[Lighting] 0:00 lightmaps preparing (then: lightmap pages, light probes, reflection probes, occlusion)",
            "[Lighting] 0:10 light probes, bounce 1 of at most 5: 0% (0/10 probes) (then: reflection probes, occlusion)",
            "[Lighting] 0:10 light probes, bounce 2 of at most 5: 0% (0/10 probes) (then: reflection probes, occlusion)",
            "[Lighting] 0:10 occlusion 0% (0/9 cells)",
        ]
    );
    assert!(!lines[0].milestone);
}

#[test]
fn an_off_log_says_no_plan_and_a_finished_log_forgets_it() {
    let mut off = BakeLog::off();
    off.plan(&Step::GENERATE);
    assert!(!off.is_on());
    let lines = logged(|log, _| {
        log.plan(&[Step::Lightmaps, Step::LightmapPages]);
        log.observe(lightmaps(0, 0, (0, 0)));
        log.finish();
        log.observe(lightmaps(0, 0, (0, 0)));
    });
    assert_eq!(
        lines.last().expect("lines").text,
        "[Lighting] 0:00 lightmaps preparing"
    );
}
