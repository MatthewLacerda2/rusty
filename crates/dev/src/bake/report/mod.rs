//! stored-format: the bake reports, `<scene>.bake/<kind>.json`

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

pub mod lightmaps;
pub mod navmesh;
pub mod occlusion;
pub mod probes;
pub mod reflections;
mod stages;

pub use stages::Stages;

#[cfg(test)]
mod tests;

pub const DIR_SUFFIX: &str = ".bake";

pub const TOP: usize = 20;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Kind {
        Lightmaps = "lightmaps",
        Probes = "probes",
        Reflections = "reflections",
        Occlusion = "occlusion",
        Navmesh = "navmesh",
    }
}

pub fn report_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{DIR_SUFFIX}"))
}

pub fn is_report_dir(name: &str) -> bool {
    let scene = name.strip_suffix(DIR_SUFFIX);
    scene.is_some_and(|s| {
        rusty_core::paths::has_extension(Path::new(s), rusty_scene::SCENE_EXTENSION)
    })
}

pub fn in_report_dir(rel: &Path) -> bool {
    (rel.parent().into_iter().flat_map(Path::components))
        .any(|c| is_report_dir(&c.as_os_str().to_string_lossy()))
}

pub fn report_path(scene_path: &str, kind: Kind) -> PathBuf {
    report_dir(scene_path).join(format!("{}.json", kind.name()))
}

#[derive(Debug)]
pub struct Report {
    kind: Kind,
    fields: Map<String, Value>,
}

impl Report {
    pub fn new(
        kind: Kind,
        stages: &Stages,
        settings: impl Serialize,
        fingerprint: u64,
        output_bytes: u64,
    ) -> Self {
        let common = json!({
            "kind": kind.name(),
            "engine": rusty_core::project::engine_commit(),
            "timestamp": unix_seconds(),
            "threads": workers(kind),
            "wallSeconds": stages.total().as_secs_f64(),
            "stages": stages.to_json(),
            "settings": serde_json::to_value(settings).unwrap_or(Value::Null),
            "inputFingerprint": format!("{fingerprint:016x}"),
            "outputBytes": output_bytes,
        });
        let Value::Object(fields) = common else {
            unreachable!("json! of an object literal is an object")
        };
        Self { kind, fields }
    }

    pub fn with(mut self, body: Value) -> Self {
        if let Value::Object(body) = body {
            self.fields.extend(body);
        }
        self
    }

    pub fn to_json(&self) -> Value {
        Value::Object(self.fields.clone())
    }

    pub fn write(&self, scene_path: &str) -> Result<PathBuf, String> {
        let dir = report_dir(scene_path);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = report_path(scene_path, self.kind);
        let text = serde_json::to_string_pretty(&self.fields).map_err(|e| e.to_string())?;
        // lint: allow(asset-write) a dev-only bake report, never read by a cache or at runtime
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(path)
    }
}

pub fn keep(scene_path: &str, report: Report) {
    if let Err(e) = report.write(scene_path) {
        log::warn!(
            "[BakeReport] could not write the {} report: {e}",
            report.kind.name()
        );
    }
}

pub fn read(scene_path: &str, kind: Kind) -> Option<Value> {
    let text = std::fs::read_to_string(report_path(scene_path, kind)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn bytes_under(path: &std::path::Path) -> u64 {
    let Ok(meta) = std::fs::metadata(path) else {
        return 0;
    };
    if meta.is_file() {
        return meta.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries.flatten().map(|e| bytes_under(&e.path())).sum()
}

pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

pub fn workers(kind: Kind) -> Value {
    match kind {
        Kind::Lightmaps | Kind::Occlusion => json!(threads()),
        Kind::Navmesh => json!(1),
        Kind::Probes | Kind::Reflections => json!("gpu"),
    }
}

pub fn drawn_centre(scene: &rusty_scene::Scene, id: u32) -> Option<glam::Vec3> {
    let world = &scene.world;
    let (min, max) = world.mesh(id)?.world_aabb(glam::Mat4::IDENTITY)?;
    let (min, max) = rusty_render::transform_aabb(min, max, world.local_to_world(id));
    Some((min + max) * 0.5)
}

fn unix_seconds() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.map_or(0, |d| d.as_secs())
}

pub fn secs(d: std::time::Duration) -> f64 {
    (d.as_secs_f64() * 1000.0).round() / 1000.0
}

pub fn point(p: glam::Vec3) -> Value {
    let r = |v: f32| ((v as f64) * 100.0).round() / 100.0;
    json!([r(p.x), r(p.y), r(p.z)])
}

pub fn entity_name(scene: &rusty_scene::Scene, id: u32) -> String {
    match scene.world.name(id) {
        Some(n) => n.to_string(),
        None => format!("#{id}"),
    }
}
