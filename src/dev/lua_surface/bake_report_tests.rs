//! `Lighting.GetBakeReport` / `Navigation.GetBakeReport` (#1980): `nil` before a
//! bake, the file's table after one, and an unknown kind refused.

use crate::dev::session::Session;

#[test]
fn a_bake_report_reads_back_as_a_table() {
    let s = Session::new("").expect("an empty session boots");
    assert_eq!(
        s.eval("return tostring(Navigation.GetBakeReport())")
            .unwrap(),
        "nil"
    );
    let path = crate::test_temp::lua_path("level.scene");
    s.eval(&format!("Scene.Save('{path}')")).unwrap();
    assert_eq!(
        s.eval("return tostring(Navigation.GetBakeReport())")
            .unwrap(),
        "nil"
    );
    assert_eq!(
        s.eval("return tostring(Navigation.Bake())").unwrap(),
        "true"
    );
    let read = "local r = Navigation.GetBakeReport() \
        return r.kind .. ' ' .. type(r.islands.count) .. ' ' .. r.stages[1].name";
    assert_eq!(s.eval(read).unwrap(), "navmesh number bake");
    // A field the bake could not measure is `nil`, not a JSON null sentinel.
    assert_eq!(
        s.eval("return tostring(Lighting.GetBakeReport('occlusion'))")
            .unwrap(),
        "nil"
    );
}

#[test]
fn an_unknown_kind_is_refused() {
    let s = Session::new("").expect("an empty session boots");
    for kind in ["bogus", "navmesh"] {
        let err = s
            .eval(&format!("return Lighting.GetBakeReport('{kind}')"))
            .unwrap_err();
        assert!(err.contains("unknown kind"), "{err}");
    }
}
