//! A play-mode `Scene.Load` runs prepare then activate, each step under the
//! installed `LoadProbe` (#2123): every step once, in order, and one `finish` naming
//! the scene swapped in, or none when the file could not be read.

use std::cell::RefCell;
use std::rc::Rc;

use super::fixture::{world_at, write_scenes};
use crate::app::LoadProbe;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// What the probe saw: the steps it ran, then `finish:<path or ->` per load.
#[derive(Clone, Default)]
struct Seen(Rc<RefCell<Vec<String>>>);

impl LoadProbe for Seen {
    fn step(&mut self, name: &'static str, step: &mut dyn FnMut()) {
        step();
        self.0.borrow_mut().push(name.to_string());
    }

    fn finish(&mut self, loaded: Option<&str>, _: &mut ConsoleLogs) {
        let tail = loaded.map_or("-", |p| p.rsplit('/').next().unwrap_or(p));
        self.0.borrow_mut().push(format!("finish:{tail}"));
    }
}

#[test]
fn a_load_runs_each_step_once_in_order_then_finishes() {
    let (menu, _) = write_scenes("probe_steps");
    let (mut gw, _) = world_at(&menu);
    let seen = Seen::default();
    gw.set_load_probe(Some(Box::new(seen.clone())));
    gw.set_playing(true);
    gw.tick(DT); // the menu's first Update queues the load; the tail swaps it in
    let steps = [
        "read", "meshes", "sidecars", "teardown", "swap", "physics", "navmesh", "sources",
    ];
    let mut want: Vec<String> = steps.iter().map(|s| s.to_string()).collect();
    want.push("finish:level.scene".to_string());
    assert_eq!(*seen.0.borrow(), want);
    assert!(gw.scene().borrow().find_entity_by_name("Level").is_some());
}

#[test]
fn a_load_that_cannot_read_its_file_finishes_with_nothing_loaded() {
    let (menu, _) = write_scenes("probe_missing");
    let (mut gw, _) = world_at(&menu);
    let seen = Seen::default();
    gw.set_load_probe(Some(Box::new(seen.clone())));
    gw.set_playing(true);
    gw.tick(DT); // the menu's own load swaps the level in
    gw.scene()
        .borrow_mut()
        .request_load("rusty_2123_no_such_dir/gone.scene".into());
    gw.tick(DT);
    let seen = seen.0.borrow();
    assert_eq!(seen.last().map(String::as_str), Some("finish:-"));
    assert_eq!(seen[seen.len() - 2], "read", "nothing past the read ran");
    assert!(gw.scene().borrow().find_entity_by_name("Level").is_some());
}
