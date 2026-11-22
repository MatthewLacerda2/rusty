use std::cell::RefCell;
use std::rc::Rc;

use super::fixture::{world_at, write_scenes};
use crate::LoadProbe;
use rusty_script::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

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
    gw.tick(DT);
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
    gw.tick(DT);
    gw.scene()
        .borrow_mut()
        .request_load("rusty_2123_no_such_dir/gone.scene".into());
    gw.tick(DT);
    let seen = seen.0.borrow();
    assert_eq!(seen.last().map(String::as_str), Some("finish:-"));
    assert_eq!(seen[seen.len() - 2], "read", "nothing past the read ran");
    assert!(gw.scene().borrow().find_entity_by_name("Level").is_some());
}

struct Twice;

impl LoadProbe for Twice {
    fn step(&mut self, _: &'static str, step: &mut dyn FnMut()) {
        step();
        step();
    }

    fn finish(&mut self, _: Option<&str>, _: &mut ConsoleLogs) {}
}

#[test]
fn a_probe_that_calls_a_step_twice_runs_it_once() {
    let (menu, _) = write_scenes("probe_twice");
    let (mut gw, console) = world_at(&menu);
    gw.set_load_probe(Some(Box::new(Twice)));
    gw.set_playing(true);
    gw.tick(DT);
    assert!(gw.scene().borrow().find_entity_by_name("Level").is_some());
    let disables = super::fixture::logs(&console)
        .iter()
        .filter(|l| l.contains("menu:disable"))
        .count();
    assert_eq!(disables, 1, "the teardown ran once");
}

#[test]
fn entering_play_runs_each_step_once_under_its_own_probe() {
    let (menu, _) = write_scenes("probe_entry");
    let (mut gw, _) = world_at(&menu);
    *gw.script_manager().scene_path_cell().borrow_mut() = Some(menu.clone());
    let (entry, load) = (Seen::default(), Seen::default());
    gw.set_entry_probe(Some(Box::new(entry.clone())));
    gw.set_load_probe(Some(Box::new(load.clone())));
    gw.set_playing(true);
    gw.tick(DT);
    let want = [
        "refresh",
        "scripts",
        "physics",
        "startup",
        "finish:menu.scene",
    ];
    assert_eq!(*entry.0.borrow(), want);
    assert_eq!(
        load.0.borrow()[0],
        "read",
        "the scene load saw none of Play's entry"
    );
}
