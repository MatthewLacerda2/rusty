//! `ApiCells::attach` is a hand-kept list of `bind` calls (#1709): this pins it to
//! `api::ApiCtx`, so a cell left unbound, or two cells of one type (the later
//! `bind` would overwrite the earlier), fails here instead of at a script's call.

use std::rc::Rc;

use mlua::Lua;

use super::lifecycle::manager_with_entity;
use crate::api::ApiCtx;
use crate::scripting::shared::ApiCells;

/// Both destructurings are exhaustive, so a cell added to either struct and not
/// to this list fails to compile.
macro_rules! every_cell_is_its_own {
    ($cells:expr, $lua:expr; $($f:ident),* $(,)?) => {{
        let ApiCtx { $($f),* } = ApiCtx::default();
        let ApiCells { $($f: _),* } = $cells;
        $(
            let got = $f.get($lua).expect(stringify!($f));
            assert!(Rc::ptr_eq(&got, &$cells.$f), "{} reaches another cell", stringify!($f));
        )*
    }};
}

#[test]
fn every_api_handle_reaches_its_own_attached_cell() {
    let (m, _) = manager_with_entity();
    let lua = Lua::new();
    m.cells.attach(&lua);
    every_cell_is_its_own!(&m.cells, &lua;
        scene, input, nav, camera, time, physics, console, storage, graphics, video,
        scene_path, audio, random, screen, application, stats, timers, event_system,
        queries);
}

/// No registrar reads a cell at registration (#175's rule, kept by #1709): a
/// function that did would capture the `Rc` it got. With nothing attached, any
/// such read fails, so registering the whole surface must succeed.
#[test]
fn registering_the_surface_reads_no_cell() {
    let lua = Lua::new();
    crate::api::register(&lua, &ApiCtx::default()).unwrap();
    let err = lua.load("return Time.time()").eval::<f64>().unwrap_err();
    assert!(err.to_string().contains("attached"), "{err}");
}
