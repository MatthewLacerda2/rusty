//! The schedule probe (#433): it watches every per-frame system exactly once, under
//! the system's short function name, is told when the frame ends, and leaves the sim
//! untouched — the property that keeps the dev layer's wall-clock timer out of the
//! determinism rule's way.

use std::cell::RefCell;
use std::rc::Rc;

use super::schedule_tests::bare;
use super::{build, Resources, Stage, SystemProbe, World};

type Log = Rc<RefCell<Vec<String>>>;

/// Records what it saw instead of timing it.
struct Recorder(Log);

impl SystemProbe for Recorder {
    fn run(&mut self, stage: Stage, name: &'static str, system: &mut dyn FnMut()) {
        self.0.borrow_mut().push(format!("{stage:?}:{name}"));
        system();
    }

    fn end_frame(&mut self, _world: &World, res: &Resources) {
        self.0
            .borrow_mut()
            .push(format!("end@{}", res.play_frame()));
    }
}

fn probed_frame(log: &Log) -> (World, Resources) {
    let (mut w, mut r) = bare();
    r.is_playing = true;
    r.probe = Some(Box::new(Recorder(Rc::clone(log))));
    build().into_schedule().run_frame(&mut w, &mut r);
    (w, r)
}

#[test]
fn probe_sees_each_system_once_by_name_then_the_frame_end() {
    let log = Log::default();
    let (_w, r) = probed_frame(&log);
    let log = log.borrow();
    assert!(
        log.contains(&"FixedUpdate:update_scripts".to_string()),
        "{log:?}"
    );
    assert!(log.contains(&"LateUpdate:layout_ui".to_string()), "{log:?}");
    let systems = log.iter().filter(|l| !l.starts_with("end@")).count();
    let registered: usize = Stage::FRAME_ORDER
        .iter()
        .map(|s| build().into_schedule().system_names(*s).len())
        .sum();
    assert_eq!(systems, registered, "every system ran exactly once");
    assert_eq!(log.last().map(String::as_str), Some("end@1"));
    assert!(r.probe.is_some(), "the probe is put back after the frame");
}

#[test]
fn a_probe_does_not_change_the_sim() {
    let (w_probed, r_probed) = probed_frame(&Log::default());
    let (mut w, mut r) = bare();
    r.is_playing = true;
    build().into_schedule().run_frame(&mut w, &mut r);
    assert_eq!(r.play_frame(), r_probed.play_frame());
    let data = |w: &World| {
        serde_json::to_string(&crate::scene::to_scene_data(&w.scene.borrow())).expect("json")
    };
    assert_eq!(
        data(&w),
        data(&w_probed),
        "same world with and without a probe"
    );
}
