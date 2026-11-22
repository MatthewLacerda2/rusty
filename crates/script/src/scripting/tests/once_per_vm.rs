use std::cell::RefCell;
use std::rc::Rc;

use super::timers::{counted, rig, tick};
use rusty_core::storage::Storage;

#[test]
fn a_namespace_change_lasts_past_its_phase() {
    let code = "_G.__seen = 'unset'\nlocal M = {}\n\
                function M.Start(id) Time.custom = 7 end\n\
                function M.Update(id) __seen = Time.custom end\nreturn M";
    let (mut m, _) = rig("once_table", code);
    tick(&mut m, 2);
    assert_eq!(m.eval("__seen").unwrap(), "7");
    assert_eq!(m.eval("type(Time.deltaTime)").unwrap(), "function");
}

#[test]
fn a_stored_api_function_works_in_a_later_phase() {
    let code = "_G.__frames = -1\nlocal M = {}\n\
                function M.Start(id) _G.count = Time.frameCount end\n\
                function M.Update(id) __frames = count() end\nreturn M";
    let (mut m, _) = rig("once_stored", code);
    tick(&mut m, 3);
    let frames: i64 = m.eval("__frames").unwrap().parse().unwrap();
    assert!(frames > 0, "the stored function ran: {frames}");
    let errors = m.cells.console.borrow().messages.len();
    assert_eq!(errors, 0, "{:?}", m.cells.console.borrow().messages);
}

#[test]
fn a_hook_reentering_the_api_through_a_coroutine_holds_no_borrow() {
    let start = "Timer.StartCoroutine(id, function(e) \
                     Timer.StartCoroutine(e, function(f) __log = __log .. 'inner,' end) \
                     Timer.Invoke(e, 'Beat', 0) \
                     __log = __log .. Scene.GetName(e) .. ',' \
                     coroutine.yield() \
                     __log = __log .. 'resumed,' \
                 end)";
    let (mut m, _) = rig("once_reenter", &counted(start));
    tick(&mut m, 3);
    assert_eq!(m.eval("__log").unwrap(), "inner,E,resumed,2,");
    let messages = &m.cells.console.borrow().messages;
    assert!(messages.is_empty(), "{messages:?}");
}

#[test]
fn a_cell_swapped_on_a_live_vm_is_reattached() {
    let (mut m, _) = rig("once_swap", &counted(""));
    let store = Rc::new(RefCell::new(Storage::default()));
    m.set_storage(Rc::clone(&store));
    m.eval("Storage.Set('save', 'level', 3)").unwrap();
    assert!(store.borrow().get("save", "level").is_some());
}
