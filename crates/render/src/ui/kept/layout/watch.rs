//! crates/render/src/ui/kept/layout/watch.rs — the change cursors a kept layout reads
//! (`layout`'s module docs). A channel nothing stamped since the last read is asked
//! and skipped (`kept::ids`, [`World::quiet`]), so a still frame collects no id.

use super::super::ids;
use rusty_components::{
    CanvasComponent, ImageComponent, LayoutElementComponent, LayoutGroupComponent,
    RectTransformComponent, TextComponent,
};
use rusty_ecs::{Activity, Children, Cursor, World};

/// The cursors of every column the layout reads.
pub(super) struct Watch {
    rect: Cursor<RectTransformComponent>,
    activity: Cursor<Activity>,
    children: Cursor<Children>,
    group: Cursor<LayoutGroupComponent>,
    element: Cursor<LayoutElementComponent>,
    image: Cursor<ImageComponent>,
    text: Cursor<TextComponent>,
    /// Read only for whether it was written: the canvases are compared whole.
    canvas: Cursor<CanvasComponent>,
}

/// The ids each channel stamped since the last read (empty for a quiet one).
pub(super) struct Written {
    /// RectTransform: an element written, placed or removed, or one joining.
    pub(super) rect: Vec<u32>,
    /// Activity, child lists, LayoutGroup and LayoutElement.
    pub(super) hierarchy: Vec<u32>,
    pub(super) activity: Vec<u32>,
    /// Image and Text.
    pub(super) content: Vec<u32>,
    /// Whether any Canvas was written (its placement, or what the rects read).
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

    /// Every cursor read (each current after), or `None` when one cannot say.
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
