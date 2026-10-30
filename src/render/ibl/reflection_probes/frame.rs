//! Choosing each frame's resident reflection probes (#1206): the baked probes whose
//! boxes are nearest the primary camera (#1238), as many as the scene's byte budget
//! holds (#1584), each judged by its cube's header before the cube is read.

use glam::Vec3;

use super::{ProbeArray, MAX_REFLECTION_PROBES, SPARE_SLOTS};
use crate::render::ibl::cubemap::{read_cube_shape, CubeShape, Refusal};
use crate::render::Renderer;
use crate::scene::{ReflectionProbe, Scene};

impl Renderer {
    /// Make the baked probes whose boxes are nearest `camera_pos` this frame's
    /// resident probes (#1206, #1238), and mark group 0 for a rebind when the array
    /// texture was replaced. They are taken nearest first while the array, with its
    /// [`SPARE_SLOTS`], stays within the scene's budget
    /// (`LightingSettings::reflections`, #1584): a cube that would put it past the
    /// budget is passed over, and nearer ones that fit are still taken, so one big
    /// cube never empties the set; the nearest is always taken, whatever its size.
    /// A cube is judged by its KTX2 header ([`read_cube_shape`]), read once per path,
    /// and none is read at all once the array is full; only the cubes the array must
    /// copy in are read whole, and dropped once copied. Cubes that don't fit the
    /// array are passed over and never uploaded (#1245). Which probes are resident
    /// depends on the camera's distance to each box; only ties are broken by scene
    /// index, and the residents are handed over in scene order, so two runs from one
    /// camera agree. A cube that fails to load is refused until it is re-written, and
    /// the frame's probes are chosen again without it, so no slot is bound to what it
    /// held before (#1609).
    pub(crate) fn update_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) {
        // Each pass that fails refuses one more cached shape, so this ends.
        while !self.place_reflection_probes(scene, camera_pos) {}
    }

    /// One pass of [`Self::update_reflection_probes`]: false when a cube it had to
    /// load was refused.
    fn place_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) -> bool {
        let probes = &scene.reflection_probes.probes;
        let budget = scene.lighting_settings.reflections.budget_bytes();
        let mut wanted = Vec::new();
        let (mut face, mut paths) = (0, Vec::new());
        for i in ranked(probes, camera_pos) {
            if wanted.len() == MAX_REFLECTION_PROBES {
                break;
            }
            let path = probes[i].cubemap_path.as_str();
            let cubes = paths.len() + usize::from(!paths.contains(&path)) + SPARE_SLOTS;
            let over = |size: u32| !paths.is_empty() && array_bytes(face.max(size), cubes) > budget;
            if over(0) {
                continue; // full: no cube is read
            }
            let Some(shape) = self.reflection_probes.shape(path) else {
                continue;
            };
            if over(shape.size) {
                continue;
            }
            face = face.max(shape.size);
            paths = push_new(paths, path);
            wanted.push((i, shape));
        }
        wanted.sort_by_key(|&(i, _)| i);
        let wanted: Vec<_> = wanted.into_iter().map(|(i, s)| (&probes[i], s)).collect();
        let (copies, rebuilt) = self.reflection_probes.place(&self.device, &wanted, budget);
        let (mut loaded, mut placed) = (Vec::new(), true);
        for (path, shape, slot, shift) in copies {
            #[cfg(test)]
            {
                self.reflection_probes.loads += 1;
            }
            match self.load_cubemap_mips(&path, shape) {
                Ok(cube) => loaded.push((cube, slot, shift)),
                Err(why) => {
                    self.reflection_probes.refuse(&path, why);
                    placed = false;
                }
            }
        }
        (self.reflection_probes).fill(&self.device, &self.queue, &loaded);
        self.global_bind_group_dirty |= rebuilt;
        placed
    }
}

impl ProbeArray {
    /// The shape of the cube at `path`, from its header the first time it is asked
    /// for (#1584); `None` for a file that is missing, malformed (#1609) or one the
    /// array does not admit, which warns once.
    fn shape(&mut self, path: &str) -> Option<CubeShape> {
        if let Some(&known) = self.shapes.get(path) {
            return known;
        }
        match read_cube_shape(path) {
            Ok(shape) if Self::admits(path, shape.size) => {
                self.shapes.insert(path.to_string(), Some(shape));
                Some(shape)
            }
            Ok(_) => {
                self.shapes.insert(path.to_string(), None);
                None
            }
            Err(why) => {
                self.refuse(path, why);
                None
            }
        }
    }

    /// Leave the cube at `path` out until it is re-written ([`Self::forget`]), with
    /// one warning saying `why`: its slot is emptied and its shape cached as `None`.
    fn refuse(&mut self, path: &str, why: Refusal) {
        log::warn!("[Reflection] cubemap {path} is left out: {why}");
        self.table.forget(|p| p == path);
        self.shapes.insert(path.to_string(), None);
    }
}

/// The bytes an array of `cubes` cubes at a `face`-texel face takes: six RGBA8
/// faces each, and a third more for the mips.
pub(crate) fn array_bytes(face: u32, cubes: usize) -> u64 {
    let level = u64::from(face) * u64::from(face) * 4 * 6;
    level * 4 / 3 * cubes as u64
}

/// `paths` with `path` added, if it is not there yet.
fn push_new<'a>(mut paths: Vec<&'a str>, path: &'a str) -> Vec<&'a str> {
    if !paths.contains(&path) {
        paths.push(path);
    }
    paths
}

/// The indices of the baked probes, nearest box to `camera_pos` first (zero inside
/// it), ties to the lower scene index, so the order is the same every run.
fn ranked(probes: &[ReflectionProbe], camera_pos: Vec3) -> Vec<usize> {
    let mut order: Vec<usize> = (0..probes.len())
        .filter(|&i| !probes[i].cubemap_path.is_empty())
        .collect();
    let distance = |i: usize| probes[i].box_distance_sq(camera_pos);
    order.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(a.cmp(&b)));
    order
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;

#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "refuse_tests.rs"]
mod refuse_tests;
