//! `Line`: points, flags and style through the API, and the scene round-trip.

use std::cell::RefCell;

use glam::Vec3;
use rusty::components::{LineComponent, TextureMode};
use rusty::scene::authoring::{add_component, ComponentKind};
use rusty::scene::Scene;

use super::{eval, round_trip, with_api};

#[test]
fn points_write_through_and_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Laser".to_string());
    assert!(add_component(
        &mut scene,
        id,
        ComponentKind::parse("line").unwrap()
    ));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Line.SetPositions({id}, {{{{0, 0, 0}}, {{1, 2, 3}}, {{0/0, 0, 0}}, {{4, 5, 6}}}})
             Line.SetPosition({id}, 0, -1, -1, -1)
             Line.SetPosition({id}, 9, 7, 7, 7)
             Line.SetUseWorldSpace({id}, false)
             Line.SetLoop({id}, true)"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "local p = Line.GetPositions({id})
                 return Line.GetPositionCount({id}), p[3][3], Line.GetUseWorldSpace({id}),
                        Line.GetLoop({id}), Line.GetPositionCount(99)"
            ),
        );
        assert_eq!(
            got,
            "Integer(3), Number(6), Boolean(false), Boolean(true), Integer(0)"
        );
        let first = eval(lua, &format!("return Line.GetPosition({id}, 0)"));
        assert_eq!(first, "Number(-1), Number(-1), Number(-1)");
        let missing = eval(lua, &format!("return Line.GetPosition({id}, 5)"));
        assert_eq!(missing, "Number(0), Number(0), Number(0)");
        lua.load(format!("Line.SetPositionCount({id}, 5)"))
            .exec()
            .unwrap();
        let grown = eval(lua, &format!("return Line.GetPosition({id}, 4)"));
        assert_eq!(
            grown, "Number(4), Number(5), Number(6)",
            "growing repeats the last"
        );
    });
    let line = scene.borrow().world.line(id).map(|l| l.clone()).unwrap();
    assert_eq!(line.positions.len(), 5);
    assert_eq!(line.positions[1], Vec3::new(1.0, 2.0, 3.0));
}

#[test]
fn style_verbs_share_one_surface_and_the_line_round_trips() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Arc".to_string());
    scene.world.set_line(id, Some(LineComponent::default()));
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Line.SetWidthCurve({id}, {{{{0, 0.25}}, {{0.5, -1}}, {{1, 0.125}}}})
             Line.SetColor({id}, 0, 1, 0)
             Line.SetTextureMode({id}, 'stretch')
             Line.SetTextureMode({id}, 'wrap')
             Line.SetTexture({id}, 'dash.png')
             Line.SetTexture({id}, nil)"
        ))
        .exec()
        .unwrap();
        let got = eval(
            lua,
            &format!(
                "return Line.GetWidth({id}), Line.GetWidth({id}, 0.5), Line.GetWidth({id}, 1),
                 Line.GetTextureMode({id}), Line.GetTexture({id}), Line.GetBlend({id})"
            ),
        );
        let want =
            r#"Number(0.25), Number(0), Number(0.125), String("Stretch"), Nil, String("Alpha")"#;
        assert_eq!(got, want, "widths clamp to ≥ 0");
        let colour = eval(lua, &format!("return Line.GetColor({id}, 0.3)"));
        assert_eq!(colour, "Number(0), Number(1), Number(0), Number(1)");
    });
    let before = scene.borrow().world.line(id).map(|l| l.clone()).unwrap();
    assert_eq!(before.style.texture_mode, TextureMode::Stretch);
    let loaded = round_trip(&scene.borrow(), "line");
    assert_eq!(loaded.world.line(id).map(|l| l.clone()), Some(before));
}
