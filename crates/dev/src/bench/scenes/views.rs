//! Where `make bench-scenes` looks from: deterministic viewpoints no one authors.
//!
//! At most [`MAX_POINTS`] positions, in order: every active screen camera the
//! scene holds, at its own transform, by entity id (past [`MAX_POINTS`] the rest
//! are left out, and the report says so, as it does when the cameras fill every
//! position and the scene has a spawn or a navmesh); the player's spawn (the entity named
//! `PlayerSpawn`, else `Player`), one view along its facing; then points on the
//! navmesh at eye height, each looking along four yaws (0, 90, 180 and 270°),
//! picked from a fixed seed for the positions left. A scene with no navmesh uses
//! its cameras and spawn alone; one with none of the three looks along +Z from its
//! bounds' centre.

use glam::Vec3;

use rusty_navigation::{NavigationGraph, SpanRef, NOT_WALKABLE_AREA};
use rusty_scene::camera::{active_cameras, set_view_orientation, set_view_position};
use rusty_scene::Scene;
use serde::{Deserialize, Serialize};

/// Eye height above the floor, metres.
pub const EYE_HEIGHT: f32 = 1.7;
/// The most points (cameras, the spawn, navmesh points) a scene is benched from.
pub const MAX_POINTS: usize = 8;
/// The yaws each navmesh point looks along, degrees.
pub const YAWS: [f32; 4] = [0.0, 90.0, 180.0, 270.0];
/// The navmesh points' seed: the same scene gives the same points on every run.
const SEED: u64 = 0x2294_BE7C_5CE7_E5ED;

/// Where one viewpoint looks from.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// A scene camera, as the only screen camera.
    Camera(u32),
    /// The main view moved here (yaw and pitch in degrees, `Camera`'s convention).
    Pose {
        position: Vec3,
        yaw: f32,
        pitch: f32,
    },
}

/// One viewpoint, the label its rows name it by, and where it looks from as
/// picked (a camera's at its pick: a script may move it later), which is what
/// tells two runs' viewpoints apart ([`Viewpoint::key`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Viewpoint {
    pub label: String,
    pub view: View,
    pub at: Vec3,
    /// Degrees, `Camera`'s convention.
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

    /// What a report keeps of it, to compare two runs' viewpoints by.
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

/// A viewpoint as a report keeps it: two runs benched the same viewpoint when
/// these are equal, so a navmesh point that moved, or another camera of the same
/// name, is not one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ViewKey {
    pub label: String,
    /// The camera's entity id, for a camera's view.
    pub camera: Option<u32>,
    pub at: [f32; 3],
    pub yaw: f32,
}

/// A scene's viewpoints, and what the report should say about how they were picked.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Views {
    pub views: Vec<Viewpoint>,
    pub warnings: Vec<String>,
}

/// The viewpoints for `scene` with its navmesh `nav`: see the module docs.
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

/// The warning for `cameras` screen cameras filling every position: what the
/// cap left out (the cameras past it, the spawn, the navmesh points), each only
/// when the scene has it. `None` when the cap left nothing out.
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

/// A scene without a navmesh: the views already picked (its cameras, its spawn)
/// alone, or, with neither, one view from its bounds' centre.
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

/// The spawn's view: eye height over the floor under it (its own height without
/// a floor there), along the way it faces.
fn spawn_view(scene: &Scene, nav: &NavigationGraph) -> Option<Viewpoint> {
    let id = ["PlayerSpawn", "Player"]
        .iter()
        .find_map(|name| scene.find_entity_by_name(name))?;
    let at = scene.world.world_position(id);
    let floor = nav.span_under(at).map(|s| nav.span_world(s).y);
    let position = Vec3::new(at.x, floor.map_or(at.y, |y| y + EYE_HEIGHT), at.z);
    Some(Viewpoint::pose("spawn".into(), position, facing(scene, id)))
}

/// The yaw entity `id` faces along (its local -Z), degrees.
fn facing(scene: &Scene, id: u32) -> f32 {
    let forward = scene.world.world_rotation(id) * Vec3::NEG_Z;
    forward.z.atan2(forward.x).to_degrees()
}

/// Every walkable span, in the graph's span order: walked, never collected, so a
/// big level's spans are not all held to pick a few.
fn walkable(nav: &NavigationGraph) -> impl Iterator<Item = SpanRef> + '_ {
    nav.span_refs()
        .filter(|s| nav.spans[s.index as usize].area != NOT_WALKABLE_AREA)
}

/// Up to `n` distinct walkable floors' points of the `floors` the graph has,
/// drawn as indices into [`walkable`] from [`SEED`] (SplitMix64).
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
    let point = |i| walkable(nav).nth(i).map(|s| nav.span_world(s));
    picked.into_iter().filter_map(point).collect()
}

/// The centre of every entity's world position (the origin in an empty scene).
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

/// Make `view` the one the next frame draws. A camera view turns every other
/// screen camera off and returns them, for [`restore`]; a pose moves the main view.
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

/// Turn back on the cameras [`apply`] turned off.
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
