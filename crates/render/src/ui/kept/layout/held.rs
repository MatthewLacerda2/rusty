use glam::{Quat, Vec3, Vec4};

use super::Why;
use rusty_asset::MapRef;
use rusty_components::{
    ImageType, LayoutElementComponent, LayoutGroupComponent, RectTransformComponent, TextComponent,
};
use rusty_ecs::World;
use rusty_ui::UiLayout;

type ImageKey = (Option<MapRef>, ImageType, Vec4);

#[derive(PartialEq)]
struct Content {
    text: Option<TextComponent>,
    image: Option<ImageKey>,
}

#[derive(PartialEq)]
struct Element {
    parent: Option<u32>,
    active: bool,
    rect: Option<RectTransformComponent>,
    group: Option<LayoutGroupComponent>,
    fit: Option<LayoutElementComponent>,
    children: Vec<u32>,
    content: Option<Content>,
    spin: Option<(Quat, Vec3)>,
}

impl Element {
    fn read(world: &World, id: u32, root: bool) -> Self {
        let sized = world.has_layout_element(id)
            || world
                .parent_id(id)
                .is_some_and(|p| world.has_layout_group(p));
        let content = sized.then(|| Content {
            text: world.text(id).map(|t| t.clone()),
            image: world
                .image(id)
                .map(|i| (i.texture.clone(), i.image_type, i.border)),
        });
        Self {
            parent: world.parent_id(id),
            active: world.is_active_in_hierarchy(id),
            rect: (!root)
                .then(|| world.rect_transform(id).map(|r| r.clone()))
                .flatten(),
            group: world.layout_group(id).map(|g| g.clone()),
            fit: world.layout_element(id).map(|f| f.clone()),
            children: world.children(id),
            content,
            spin: (!root)
                .then(|| world.transform(id).map(|t| (t.rotation, t.scale)))
                .flatten(),
        }
    }

    fn differs(&self, now: &Element) -> Option<Why> {
        if self.content != now.content {
            return Some(Why::Content);
        }
        if self.spin != now.spin {
            return Some(Why::Transform);
        }
        (self != now).then_some(Why::Structure)
    }
}

const HELD: u8 = 1;
const CANVAS: u8 = 2;

pub(super) struct Elements {
    held: Vec<(u32, Element)>,
    flags: Vec<u8>,
}

impl Elements {
    pub(super) fn read(world: &World, layout: &UiLayout) -> Self {
        let mut held: Vec<(u32, Element)> = layout
            .iter()
            .map(|(id, r)| (id, Element::read(world, id, r.canvas == id)))
            .collect();
        held.sort_unstable_by_key(|(id, _)| *id);
        let mut flags = Vec::new();
        let mut mark = |id: u32, flag: u8| {
            let at = id as usize;
            if at >= flags.len() {
                flags.resize(at + 1, 0);
            }
            flags[at] |= flag;
        };
        held.iter().for_each(|(id, _)| mark(*id, HELD));
        world
            .ids_with_canvas()
            .into_iter()
            .for_each(|id| mark(id, CANVAS));
        Self { held, flags }
    }

    fn flag(&self, id: u32, flag: u8) -> bool {
        self.flags.get(id as usize).is_some_and(|f| f & flag != 0)
    }

    pub(super) fn holds(&self, id: u32) -> bool {
        self.flag(id, HELD)
    }

    pub(super) fn was_canvas(&self, id: u32) -> bool {
        self.flag(id, CANVAS)
    }

    pub(super) fn moved(&self, world: &World, id: u32, joins: bool) -> Option<Why> {
        let Ok(at) = self.held.binary_search_by_key(&id, |(h, _)| *h) else {
            let parent = world.parent_id(id).filter(|&p| self.holds(p));
            return (joins && parent.is_some() && world.has_rect_transform(id))
                .then_some(Why::Structure);
        };
        let (_, held) = &self.held[at];
        held.differs(&Element::read(world, id, held.rect.is_none()))
    }

    /// Compared each frame, not watched: a Transform cursor would log every sim Transform write.
    pub(super) fn spins_still(&self, world: &World) -> bool {
        let still = |(id, e): &(u32, Element)| match e.spin {
            None => true,
            Some(spin) => world
                .transform(*id)
                .is_some_and(|t| (t.rotation, t.scale) == spin),
        };
        self.held.iter().all(still)
    }
}
