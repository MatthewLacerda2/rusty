use glam::{Mat4, Vec2, Vec3};

use rusty_ecs::World;
use rusty_scene::Camera;
use rusty_ui::UiLayout;

type Eye = (Mat4, Vec3, Vec3);

type Targets = Vec<(u32, Option<Vec3>)>;

#[derive(PartialEq)]
pub(super) struct Markers {
    targets: Targets,
    eye: Eye,
}

impl Markers {
    pub(super) fn read(
        world: &World,
        layout: &UiLayout,
        cam: &Camera,
        screen: Vec2,
    ) -> Option<Self> {
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
            let Some(anchor) = rt.world_anchor.as_ref() else {
                continue;
            };
            if world.is_active_in_hierarchy(id) && placed(id).is_some() {
                any = true;
                targets.extend(anchor.target);
            }
        }
        targets.sort_unstable();
        targets.dedup();
        let targets = targets
            .into_iter()
            .map(|t| (t, position(world, t)))
            .collect();
        any.then(|| Self {
            targets,
            eye: eye(cam, screen),
        })
    }

    pub(super) fn still(&self, world: &World, cam: &Camera, screen: Vec2) -> bool {
        let at = |&(t, p): &(u32, Option<Vec3>)| position(world, t) == p;
        self.eye == eye(cam, screen) && self.targets.iter().all(at)
    }
}

fn position(world: &World, target: u32) -> Option<Vec3> {
    world.transform(target)?;
    Some(world.local_to_world(target).transform_point3(Vec3::ZERO))
}

fn eye(cam: &Camera, screen: Vec2) -> Eye {
    let aspect = screen.x / screen.y;
    (
        cam.build_view_projection(aspect),
        cam.position,
        cam.forward(),
    )
}
