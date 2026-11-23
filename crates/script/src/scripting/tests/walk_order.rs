//! `pairs` walks a table in an order that is the same on every launch, for string keys and
//! for object keys. VMs in one process share an allocator and may agree by luck, so the
//! proof is a second process.

use std::path::Path;

use mlua::Lua;

use crate::scripting::sandbox::{inspection_vm, sandboxed_vm};

/// Set on the child process: where it writes its walks.
const OUT_VAR: &str = "RUSTY_WALK_ORDER_OUT";

/// Each line is one walk; an object key is named by the `i` it was made with.
const WALKS: &str = r#"
local out = {}
local function walk(t, name)
    local seen = {}
    for k, v in pairs(t) do seen[#seen + 1] = name(k, v) end
    out[#out + 1] = table.concat(seen, ",")
end
local words = {}
for i = 1, 64 do words["key" .. i] = i end
words.alpha, words.omega, words[("long"):rep(20)] = 1, 2, 3
walk(words, function(k) return k end)
local tables, closures, threads = {}, {}, {}
for i = 1, 64 do
    tables[{ i = i }] = i
    closures[function() return i end] = i
    threads[coroutine.create(function() return i end)] = i
end
walk(tables, function(_, v) return v end)
walk(closures, function(_, v) return v end)
walk(threads, function(_, v) return v end)
local userdata = {}
for i, u in ipairs(made) do userdata[u] = i end
walk(userdata, function(_, v) return v end)
local builtins = { print = print, type = type, pairs = pairs, next = next, select = select,
    tostring = tostring, floor = math.floor, sub = string.sub, insert = table.insert }
local by_function = {}
for name, f in pairs(builtins) do by_function[f] = name end
walk(by_function, function(_, v) return v end)
return table.concat(out, "\n")
"#;

fn walks(lua: &Lua) -> String {
    let made = lua
        .create_sequence_from((1..=64).map(|i| lua.create_any_userdata(i).unwrap()))
        .unwrap();
    lua.globals().set("made", made).unwrap();
    lua.load(WALKS).set_name("=walks").eval().unwrap()
}

fn both_vms() -> String {
    let play = walks(&sandboxed_vm().unwrap());
    let inspection = walks(&inspection_vm(Path::new("")).unwrap());
    format!("{play}\n--\n{inspection}")
}

#[test]
fn pairs_walks_in_the_same_order_in_another_process() {
    let here = both_vms();
    let out = rusty_test_support::temp::dir().join("rusty_walk_order_child.txt");
    let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "scripting::tests::walk_order::child_walks"])
        .arg("--include-ignored")
        .env(OUT_VAR, &out)
        .status()
        .expect("spawn the child process");
    assert!(status.success(), "child run failed");
    let there = std::fs::read_to_string(&out).expect("child output");
    assert_eq!(here, there, "a second process walked in another order");
}

#[test]
fn two_vms_in_one_process_walk_alike() {
    assert_eq!(both_vms(), both_vms());
}

/// The child half of the cross-process test; a no-op unless that test spawned it.
#[test]
// lint: allow(skip) the child half: pairs_walks_in_the_same_order_in_another_process runs it
#[ignore = "run by pairs_walks_in_the_same_order_in_another_process in a child process"]
fn child_walks() {
    if let Some(out) = std::env::var_os(OUT_VAR) {
        std::fs::write(out, both_vms()).expect("write child walks");
    }
}
