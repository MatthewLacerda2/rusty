//! `Text` setters write through the shared ops, validation included (#419).

use glam::{Vec2, Vec4};

use super::{label, with_api};
use std::cell::RefCell;

#[test]
fn text_setters_validate_and_write_through() {
    let (scene, id) = label();
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        lua.load(format!(
            "Text.SetText({id}, 'AMMO 30')
             Text.SetFontSize({id}, -5)
             Text.SetColor({id}, 2, 0.5, 0, 0.25)
             Text.SetAlignment({id}, 'middlecenter')
             Text.SetOverflow({id}, 'Ellipsis')
             Text.SetOverflow({id}, 'Scroll')
             Text.SetFont({id}, '')
             Text.SetAutoSize({id}, true, 40, 12)
             Text.SetOutline({id}, 0.05, 1, 0, 0, 1)
             Text.SetShadow({id}, 2, -2, 0, 0, 0, 0.5)
             Text.SetGlow({id}, -1, 0, 1, 1, 1)"
        ))
        .exec()
        .unwrap();
        let t = scene.borrow().world.text(id).unwrap().clone();
        assert_eq!(t.text, "AMMO 30");
        assert_eq!(t.font_size, 0.5, "kept positive");
        assert_eq!(t.color, Vec4::new(1.0, 0.5, 0.0, 0.25));
        assert_eq!(t.font, None, "an empty path is the default font");
        assert_eq!((t.auto_size_min, t.auto_size_max), (12.0, 40.0));
        assert_eq!(t.shadow_offset, Vec2::new(2.0, -2.0));
        assert_eq!(t.glow_size, 0.0, "widths stay non-negative");
        let names: (String, String) = lua
            .load(format!(
                "return Text.GetAlignment({id}), Text.GetOverflow({id})"
            ))
            .eval()
            .unwrap();
        assert_eq!(names, ("MiddleCenter".into(), "Ellipsis".into()));
        let outline: (f32, f32, f32) = lua
            .load(format!(
                "local w, r, g = Text.GetOutline({id}) return w, r, g"
            ))
            .eval()
            .unwrap();
        assert_eq!(outline, (0.05, 1.0, 0.0));
        let short = lua.load(format!("Text.SetGlow({id}, 0.1)")).exec();
        assert!(short.is_err(), "a missing colour is an error");
        let none: Option<String> = lua.load("return Text.GetText(999)").eval().unwrap();
        assert!(none.is_none(), "no Text: a neutral default");
    });
}
