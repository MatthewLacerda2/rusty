use glam::Vec2;

use rusty_components::RenderMode;
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::Scene;

const FLAT: f32 = 0.05;
pub const DEPTH: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Liquid {
    pub min: Vec2,
    pub max: Vec2,
    pub surface: f32,
}

pub fn liquids(scene: &Scene) -> Vec<Liquid> {
    let world = &scene.world;
    let mut out = Vec::new();
    for id in world.ids_with_mesh() {
        if !world.is_static(id) || !world.is_active_in_hierarchy(id) {
            continue;
        }
        let solid = world
            .collider(id)
            .is_some_and(|c| c.active && !c.is_trigger);
        let transparent = (world.material(id))
            .and_then(|m| scene.materials.get(m.first()))
            .is_some_and(|m| m.render_mode == RenderMode::Transparent);
        if solid || !transparent {
            continue;
        }
        let mesh = world.mesh(id).expect("id came from ids_with_mesh");
        let Some((lo, hi)) = mesh.world_aabb(world.local_to_world(id)) else {
            continue;
        };
        if hi.y - lo.y <= FLAT && (hi - lo).is_finite() {
            out.push(Liquid {
                min: Vec2::new(lo.x, lo.z),
                max: Vec2::new(hi.x, hi.z),
                surface: hi.y,
            });
        }
    }
    out
}

pub fn drown(plan: &mut FloorGrid, liquids: &[Liquid]) {
    let mut drowned: Vec<(usize, f32)> = Vec::new();
    for i in 0..plan.cells.len() {
        let centre = plan.centre(i % plan.width, i / plan.width);
        let over = |l: &&Liquid| centre.cmpge(l.min).all() && centre.cmple(l.max).all();
        for l in liquids.iter().filter(over) {
            let first = floors_of(plan, i)
                .filter(|&f| f < l.surface)
                .reduce(f32::max);
            if let Some(f) = first.filter(|&f| l.surface - f >= DEPTH) {
                drowned.push((i, f));
            }
        }
    }
    for &(i, f) in &drowned {
        if plan.cells[i].is_some_and(|c| c.floor == f) {
            plan.cells[i] = None;
        }
    }
    drowned.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    plan.upper.retain(|&(i, f)| {
        let at = |d: &(usize, f32)| d.0.cmp(&i).then(d.1.total_cmp(&f));
        drowned.binary_search_by(at).is_err()
    });
}

fn floors_of(plan: &FloorGrid, i: usize) -> impl Iterator<Item = f32> + '_ {
    let from = plan.upper.partition_point(|u| u.0 < i);
    let upper = plan.upper[from..].iter().take_while(move |u| u.0 == i);
    (plan.cells[i].map(|c| c.floor).into_iter()).chain(upper.map(|u| u.1))
}
