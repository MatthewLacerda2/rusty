//! crates/render/src/ui/kept/markers.rs — what a kept layout's `WorldAnchor` markers
//! were placed from: their targets and the camera (`layout`'s module docs).

use glam::{Mat4, Vec3};

use rusty_ecs::World;
use rusty_scene::Camera;
use rusty_ui::UiLayout;

/// What a marker reads of the camera: its view-projection on this screen, and the
/// eye and forward axis its behind test reads.
pub(super) type Eye = (Mat4, Vec3, Vec3);

/// The sorted targets of every `WorldAnchor` the layout places as a marker, or
/// `None` when it places none. `compute_in` places one only under a screen-space
/// root seen through a camera, so an anchor under a world canvas, or outside any
/// canvas, is no marker; it is found by its parent, which the layout laid out, so a
/// marker hidden this frame (left out with its subtree) still counts: a move can
/// show it.
pub(super) fn marker_targets(world: &World, layout: &UiLayout) -> Option<Vec<u32>> {
    let screen_root = |canvas: u32| world.canvas(canvas).is_some_and(|c| !c.is_world_space());
    let placed = |id: u32| {
        let parent = world.parent_id(id)?;
        layout.get(parent).filter(|r| screen_root(r.canvas))
    };
    let mut any = false;
    let mut targets = Vec::new();
    for id in world.ids_with_rect_transform() {
        let Some(rt) = world.rect_transform(id) else {
            continue;
        };
        if let Some(anchor) = rt.world_anchor.as_ref().filter(|_| placed(id).is_some()) {
            any = true;
            targets.extend(anchor.target);
        }
    }
    targets.sort_unstable();
    targets.dedup();
    any.then_some(targets)
}

/// The camera as markers read it on a `screen`-pixel target.
pub(super) fn eye(cam: &Camera, screen: glam::Vec2) -> Eye {
    let aspect = screen.x / screen.y;
    (
        cam.build_view_projection(aspect),
        cam.position,
        cam.forward(),
    )
}
