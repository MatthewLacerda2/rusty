//! `Text.GetPreferredSize` / `GetLayout` answer from the element's rect (#419).

use super::{label, with_api};
use std::cell::RefCell;

#[test]
fn layout_reads_answer_from_the_elements_rect() {
    let (scene, id) = label();
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        let text = "a subtitle long enough to wrap inside four hundred units";
        lua.load(format!(
            "Text.SetText({id}, '{text}') Text.SetFontSize({id}, 30)"
        ))
        .exec()
        .unwrap();
        let (w, h): (f32, f32) = lua
            .load(format!("return Text.GetPreferredSize({id})"))
            .eval()
            .unwrap();
        assert!(w > 400.0, "unwrapped it is wider than the rect: {w}");
        let (lines, height): (u32, f32) = lua
            .load(format!(
                "local l = Text.GetLayout({id}) return l.lines, l.height"
            ))
            .eval()
            .unwrap();
        assert!(lines >= 2);
        assert_eq!(height, h, "the preferred height is the wrapped height");
        let fitted: (f32, bool) = lua
            .load(format!(
                "Text.SetAutoSize({id}, true, 8, 60)
                 local l = Text.GetLayout({id}) return l.font_size, l.truncated"
            ))
            .eval()
            .unwrap();
        assert!(fitted.0 < 30.0 && !fitted.1, "auto-size shrinks it to fit");
    });
}
