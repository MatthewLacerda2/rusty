use glam::Vec3;

use rusty_navigation::{NavigationGraph, SpanRef, NOT_WALKABLE_AREA};
use rusty_scene::camera::{active_cameras, set_view_orientation, set_view_position};
use rusty_scene::Scene;

use super::report::ViewKey;

pub const EYE_HEIGHT: f32 = 1.7;
pub const MAX_POINTS: usize = 8;
pub const YAWS: [f32; 4] = [0.0, 90.0, 180.0, 270.0];
const SEED: u64 = 0x2294_BE7C_5CE7_E5ED;

#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Camera(u32),
    Pose {
        position: Vec3,
        yaw: f32,
        pitch: f32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Viewpoint {
    pub label: String,
    pub view: View,
    pub at: Vec3,
    pub yaw: f32,
}

impl Viewpoint {
    fn pose(label: String, position: Vec3, yaw: f32) -> Self {
        Self {
            label,
            view: View::Pose {
                position,
                yaw,
                pitch: 0.0,
            },
            at: position,
            yaw,
        }
    }

    pub fn key(&self) -> ViewKey {
        ViewKey {
            label: self.label.clone(),
            camera: match self.view {
                View::Camera(id) => Some(id),
                View::Pose { .. } => None,
            },
            at: self.at.to_array(),
            yaw: self.yaw,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Views {
    pub views: Vec<Viewpoint>,
    pub warnings: Vec<String>,
}

pub fn pick(scene: &Scene, nav: &NavigationGraph) -> Views {
    let mut out = Views::default();
    let mut cameras: Vec<u32> = active_cameras(scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| id)
        .collect();
    cameras.sort_unstable();
    let spawn = spawn_view(scene, nav);
    let floors = walkable(nav).count();
    if cameras.len() >= MAX_POINTS {
        out.warnings
            .extend(capped(cameras.len(), spawn.is_some(), floors > 0));
        cameras.truncate(MAX_POINTS);
    }
    for &id in &cameras {
        let name = scene.world.name(id).map_or(String::new(), |n| n.clone());
        let (at, yaw) = (scene.world.world_position(id), facing(scene, id));
        out.views.push(Viewpoint {
            label: format!("camera {name}"),
            view: View::Camera(id),
            at,
            yaw,
        });
    }
    let spawn = spawn.filter(|_| cameras.len() < MAX_POINTS);
    let points = cameras.len() + usize::from(spawn.is_some());
    out.views.extend(spawn);
    if floors == 0 {
        no_floor(&mut out, scene);
        return out;
    }
    for (n, floor) in seeded(nav, floors, MAX_POINTS - points)
        .into_iter()
        .enumerate()
    {
        for yaw in YAWS {
            out.views.push(Viewpoint::pose(
                format!("nav {n} yaw {yaw}"),
                floor + Vec3::Y * EYE_HEIGHT,
                yaw,
            ));
        }
    }
    out
}

fn capped(cameras: usize, spawn: bool, floors: bool) -> Option<String> {
    let extra = cameras - MAX_POINTS;
    let s = if extra == 1 { "" } else { "s" };
    let left: Vec<String> = [
        (extra > 0).then(|| format!("the other {extra} camera{s}")),
        spawn.then(|| "the spawn".into()),
        floors.then(|| "the navmesh points".into()),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (last, rest) = left.split_last()?;
    let what = match rest {
        [] => last.clone(),
        _ => format!("{} and {last}", rest.join(", ")),
    };
    Some(format!(
        "{cameras} cameras, {MAX_POINTS} benched (at most {MAX_POINTS} positions), \
         left out: {what}"
    ))
}

fn no_floor(out: &mut Views, scene: &Scene) {
    let cameras = out.views.iter().any(|v| matches!(v.view, View::Camera(_)));
    let spawn = out.views.iter().any(|v| v.label == "spawn");
    let used = match (cameras, spawn) {
        (true, true) => "the scene's cameras and its spawn are its only viewpoints",
        (true, false) => "the scene's cameras are its only viewpoints",
        (false, true) => "the spawn is its only viewpoint",
        (false, false) => {
            out.warnings.push(
                "no camera, no spawn and no navmesh: one view from the scene bounds' centre, \
                 along +Z"
                    .into(),
            );
            let label = "bounds centre".into();
            out.views
                .push(Viewpoint::pose(label, bounds_centre(scene), 90.0));
            return;
        }
    };
    out.warnings.push(format!("no navmesh: {used}"));
}

fn spawn_view(scene: &Scene, nav: &NavigationGraph) -> Option<Viewpoint> {
    let id = ["PlayerSpawn", "Player"]
        .iter()
        .find_map(|name| scene.find_entity_by_name(name))?;
    let at = scene.world.world_position(id);
    let floor = nav.span_under(at).map(|s| nav.span_world(s).y);
    let position = Vec3::new(at.x, floor.map_or(at.y, |y| y + EYE_HEIGHT), at.z);
    Some(Viewpoint::pose("spawn".into(), position, facing(scene, id)))
}

fn facing(scene: &Scene, id: u32) -> f32 {
    let forward = scene.world.world_rotation(id) * Vec3::NEG_Z;
    forward.z.atan2(forward.x).to_degrees()
}

fn walkable(nav: &NavigationGraph) -> impl Iterator<Item = SpanRef> + '_ {
    nav.span_refs()
        .filter(|s| nav.spans[s.index as usize].area != NOT_WALKABLE_AREA)
}

fn seeded(nav: &NavigationGraph, floors: usize, n: usize) -> Vec<Vec3> {
    let mut state = SEED;
    let mut picked: Vec<usize> = Vec::new();
    while picked.len() < n.min(floors) {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        let at = ((z ^ (z >> 31)) % floors as u64) as usize;
        if !picked.contains(&at) {
            picked.push(at);
        }
    }
    let reach = picked.iter().max().map_or(0, |&i| i + 1);
    let mut points = vec![None; picked.len()];
    for (i, span) in walkable(nav).enumerate().take(reach) {
        if let Some(k) = picked.iter().position(|&p| p == i) {
            points[k] = Some(nav.span_world(span));
        }
    }
    points.into_iter().flatten().collect()
}

fn bounds_centre(scene: &Scene) -> Vec3 {
    let positions = scene
        .world
        .ids()
        .iter()
        .map(|&id| scene.world.world_position(id));
    let (min, max) = positions.fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if min.x > max.x {
        Vec3::ZERO
    } else {
        (min + max) * 0.5
    }
}

pub fn apply(scene: &mut Scene, camera: &mut rusty_scene::Camera, view: &View) -> Vec<u32> {
    match view {
        View::Camera(id) => {
            let others: Vec<u32> = active_cameras(scene)
                .filter(|(other, target)| target.is_none() && other != id)
                .map(|(other, _)| other)
                .collect();
            for &other in &others {
                if let Some(mut c) = scene.world.camera_mut(other) {
                    c.active = false;
                }
            }
            others
        }
        View::Pose {
            position,
            yaw,
            pitch,
        } => {
            set_view_position(camera, scene, *position);
            set_view_orientation(camera, scene, *yaw, *pitch);
            Vec::new()
        }
    }
}

pub fn restore(scene: &mut Scene, turned_off: &[u32]) {
    for &id in turned_off {
        if let Some(mut c) = scene.world.camera_mut(id) {
            c.active = true;
        }
    }
}

#[cfg(test)]
#[path = "views_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "views_cap_tests.rs"]
mod cap_tests;
