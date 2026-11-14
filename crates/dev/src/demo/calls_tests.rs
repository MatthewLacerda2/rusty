//! `calls`: what a scenario's VM notes, and how a replay makes it again.

use super::*;

#[test]
fn a_wrapped_call_is_noted_and_a_handle_taints() {
    let lua = Lua::new();
    lua.load("Ns = { Echo = function(...) return ... end, Handle = function() return print end }")
        .exec()
        .unwrap();
    let journal = Rc::new(RefCell::new(HostJournal::default()));
    journal.borrow_mut().start();
    journal_calls(&lua, &journal).unwrap();
    let got: i64 = lua
        .load("return Ns.Echo(7, 0.5, 'a', { 1, 2 }, { k = true }, nil)")
        .eval()
        .unwrap();
    assert_eq!(got, 7, "the wrapped function still answers");
    lua.load("Ns.Handle()").exec().unwrap();
    let calls = journal.borrow_mut().take();
    let echo = HostCall::Call {
        function: "Ns.Echo".into(),
        args: vec![
            Plain::Int(7),
            Plain::Number(0.5),
            Plain::Str("a".into()),
            Plain::List(vec![Plain::Int(1), Plain::Int(2)]),
            Plain::Map(vec![("k".into(), Plain::Bool(true))]),
            Plain::Nil,
        ],
    };
    assert_eq!(calls[0], echo);
    assert!(
        matches!(&calls[2], HostCall::Taint(what) if what.contains("Ns.Handle returned a function"))
    );
}

#[test]
fn a_function_argument_taints() {
    let lua = Lua::new();
    lua.load("Ns = { Take = function() end }").exec().unwrap();
    let journal = Rc::new(RefCell::new(HostJournal::default()));
    journal.borrow_mut().start();
    journal_calls(&lua, &journal).unwrap();
    lua.load("Ns.Take(print)").exec().unwrap();
    let calls = journal.borrow_mut().take();
    assert!(matches!(&calls[..], [HostCall::Taint(what)] if what.contains("passed a function")));
}

/// Each of `forwards::FORWARDS` replays in the gameplay VM, never the replay's
/// own: the check that a verb forwarded when recorded is forwarded on replay
/// (#1929). It walks the table, so a verb added to it is held too.
#[test]
fn every_forwarded_verb_replays_in_the_gameplay_vm() {
    let out = rusty_test_support::temp::dir().join(format!("demo_forwards_{}", std::process::id()));
    let harness = crate::harness::Harness::new(out, "");
    harness.step(1);
    let world = harness.world.borrow();
    let replay = super::super::replay::replay_vm(&world).unwrap();
    let game = world.script_manager();
    // Each VM's `ns.name` stubbed: the game's counts, the replay's refuses.
    let stub = |lua: &Lua, (ns, name): (&str, &str), body: &str| {
        let code = format!(
            "local ns, name = ...; local real = _G[ns][name]; \
             _G[ns][name] = function() {body} end; return type(real)"
        );
        let real: String = lua.load(code).call((ns, name)).unwrap();
        assert_eq!(real, "function", "{ns}.{name} is an API verb");
    };
    let mut verbs = 0;
    for entry in &super::super::forwards::FORWARDS {
        let namespace = entry.namespace;
        for &name in entry.verbs {
            verbs += 1;
            game.with_vm(|glua| {
                stub(glua, (namespace, name), "Forwarded = (Forwarded or 0) + 1");
                Ok(())
            })
            .unwrap()
            .expect("the game's runtime is up");
            stub(&replay, (namespace, name), "error('made in the replay VM')");
            invoke(&replay, game, &format!("{namespace}.{name}"), &[]).unwrap();
        }
    }
    let count = game.with_vm(|glua| glua.globals().get::<i64>("Forwarded"));
    assert_eq!(count.unwrap(), Some(verbs));
}
