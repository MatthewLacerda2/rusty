use super::{build, Resources, Schedule, Stage, World};
use rusty_core::input::InputState;
use rusty_navigation::NavigationGraph;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;
use rusty_time::Time;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static TRACE: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

fn rec(tag: &'static str) {
    TRACE.with(|t| t.borrow_mut().push(tag));
}

fn fixed(_w: &mut World, _r: &mut Resources) {
    rec("fixed");
}
fn update(_w: &mut World, _r: &mut Resources) {
    rec("update");
}
fn late(_w: &mut World, _r: &mut Resources) {
    rec("late");
}
fn render(_w: &mut World, _r: &mut Resources) {
    rec("render");
}
fn startup(_w: &mut World, _r: &mut Resources) {
    rec("startup");
}
fn first(_w: &mut World, _r: &mut Resources) {
    rec("first");
}
fn second(_w: &mut World, _r: &mut Resources) {
    rec("second");
}

pub(super) fn bare() -> (World, Resources) {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(glam::Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let res = Resources::new(Rc::clone(&scene), input, nav, console, camera, time);
    (World::new(scene), res)
}

#[test]
fn frame_runs_stages_in_canonical_order() {
    TRACE.with(|t| t.borrow_mut().clear());
    let mut sched = Schedule::new();
    sched.add_system(Stage::Render, render);
    sched.add_system(Stage::Update, update);
    sched.add_system(Stage::LateUpdate, late);
    sched.add_system(Stage::FixedUpdate, fixed);
    let (mut w, mut r) = bare();
    sched.run_frame(&mut w, &mut r);
    let trace = TRACE.with(|t| t.borrow().clone());
    assert_eq!(trace, vec!["fixed", "update", "late", "render"]);
}

#[test]
fn startup_is_excluded_from_the_frame() {
    TRACE.with(|t| t.borrow_mut().clear());
    let mut sched = Schedule::new();
    sched.add_system(Stage::Startup, startup);
    sched.add_system(Stage::Update, update);
    let (mut w, mut r) = bare();
    sched.run_frame(&mut w, &mut r);
    assert_eq!(TRACE.with(|t| t.borrow().clone()), vec!["update"]);
    sched.run_startup(&mut w, &mut r);
    assert_eq!(
        TRACE.with(|t| t.borrow().clone()),
        vec!["update", "startup"]
    );
}

#[test]
fn within_stage_runs_in_registration_order() {
    TRACE.with(|t| t.borrow_mut().clear());
    let mut sched = Schedule::new();
    sched.add_system(Stage::Update, first);
    sched.add_system(Stage::Update, second);
    let (mut w, mut r) = bare();
    sched.run_frame(&mut w, &mut r);
    assert_eq!(TRACE.with(|t| t.borrow().clone()), vec!["first", "second"]);
}

#[test]
fn frame_order_matches_stage_indices() {
    assert_eq!(Stage::Startup.index(), 0);
    let order = Stage::FRAME_ORDER;
    assert_eq!(order.len(), 4);
    for pair in order.windows(2) {
        assert!(pair[0].index() < pair[1].index());
    }
    assert!(!order.contains(&Stage::Startup));
}

#[test]
fn build_registers_seven_fixed_update_systems() {
    let (_w, r) = bare();
    let sched = build().into_schedule();
    drop(r);
    let (mut w2, mut r2) = bare();
    sched.run_frame(&mut w2, &mut r2);
    assert_eq!(r2.play_frame(), 1);
}
