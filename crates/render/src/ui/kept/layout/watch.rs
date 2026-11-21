use super::super::ids;
use rusty_components::{
    CanvasComponent, ImageComponent, LayoutElementComponent, LayoutGroupComponent,
    RectTransformComponent, TextComponent,
};
use rusty_ecs::{Activity, Children, Cursor, World};

pub(super) struct Watch {
    rect: Cursor<RectTransformComponent>,
    activity: Cursor<Activity>,
    children: Cursor<Children>,
    group: Cursor<LayoutGroupComponent>,
    element: Cursor<LayoutElementComponent>,
    image: Cursor<ImageComponent>,
    text: Cursor<TextComponent>,
    canvas: Cursor<CanvasComponent>,
}

pub(super) struct Written {
    pub(super) rect: Vec<u32>,
    pub(super) hierarchy: Vec<u32>,
    pub(super) activity: Vec<u32>,
    pub(super) content: Vec<u32>,
    pub(super) canvas: bool,
}

impl Watch {
    pub(super) fn open(w: &World) -> Self {
        Self {
            rect: w.change_cursor(),
            activity: w.change_cursor(),
            children: w.change_cursor(),
            group: w.change_cursor(),
            element: w.change_cursor(),
            image: w.change_cursor(),
            text: w.change_cursor(),
            canvas: w.change_cursor(),
        }
    }

    pub(super) fn read(&mut self, w: &World) -> Option<Written> {
        let all = [
            ids(w, &mut self.rect),
            ids(w, &mut self.activity),
            ids(w, &mut self.children),
            ids(w, &mut self.group),
            ids(w, &mut self.element),
            ids(w, &mut self.image),
            ids(w, &mut self.text),
            ids(w, &mut self.canvas),
        ];
        let [rect, activity, children, group, element, image, text, canvas] = all;
        let mut hierarchy = children?;
        hierarchy.extend(group?.into_iter().chain(element?));
        let mut content = image?;
        content.extend(text?);
        Some(Written {
            rect: rect?,
            hierarchy,
            activity: activity?,
            content,
            canvas: !canvas?.is_empty(),
        })
    }
}
