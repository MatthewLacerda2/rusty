//! Bake reports (#1980): every bake writes what it measured and what it found
//! beside the scene, one JSON file per bake kind, `<scene>.bake/<kind>.json`,
//! overwritten by the next bake of that kind. Unity's bake prints a few lines and
//! a total that scroll away; a report stays, so two bakes can be compared and a
//! scenario can assert on one (`Lighting.GetBakeReport(kind)`,
//! `Navigation.GetBakeReport()`).
//!
//! Every report opens with the same block ([`Report::new`]): the engine commit
//! (as the project's engine check reads it, `core::project::engine_commit`), the
//! Unix time it was written, what the bake ran on ([`workers`]), the wall time per stage and
//! their total, the settings the bake used, a fingerprint of what it read, and the
//! bytes it wrote. Then each kind adds its own findings (`lightmaps`, `probes`,
//! `reflections`, `occlusion`, `navmesh`): the heaviest meshes, colliders and
//! occluders, the islands a navmesh fell into, which reflection probe each object
//! picks. Keys are camelCase, as the Lua surface's tables are.
//!
//! Dev-only and never read at runtime: only a bake writes one (never a scene save),
//! the player never opens one, and an export leaves the folder behind
//! (`dev::export::copy::left_behind`); a test game's determinism check leaves
//! the reports out of its comparison too ([`in_report_dir`]). The clock is read here, the platform side;
//! the bakes themselves stay pure. A report that cannot be written is a warning,
//! never a failed bake ([`keep`]).
//!
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

/// Suffix of the folder beside a scene file that holds its bake reports.
pub const DIR_SUFFIX: &str = ".bake";

/// How many entries a "heaviest first" list keeps.
pub const TOP: usize = 20;

rusty_core::named_enum! {
    /// The bakes that write a report; a name is the report's file stem and what
    /// `Lighting.GetBakeReport` takes.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Kind {
        Lightmaps = "lightmaps",
        Probes = "probes",
        Reflections = "reflections",
        Occlusion = "occlusion",
        Navmesh = "navmesh",
    }
}

/// The report folder for a scene: `foo.scene` → `foo.scene.bake/`.
pub fn report_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!("{scene_path}{DIR_SUFFIX}"))
}

/// Whether a folder named `name` holds a scene's bake reports: `<scene
/// file>.bake`, as [`report_dir`] names it. Another folder ending in `.bake`
/// (`bread.bake/`) is the game's own: it ships and is compared.
pub fn is_report_dir(name: &str) -> bool {
    let scene = name.strip_suffix(DIR_SUFFIX);
    scene.is_some_and(|s| {
        rusty_core::paths::has_extension(Path::new(s), rusty_scene::SCENE_EXTENSION)
    })
}

/// Whether the relative path `rel` is a file inside a scene's report folder
/// ([`is_report_dir`]): what an export leaves behind and a test game's
/// determinism check leaves out, since a report is a measurement by design.
pub fn in_report_dir(rel: &Path) -> bool {
    (rel.parent().into_iter().flat_map(Path::components))
        .any(|c| is_report_dir(&c.as_os_str().to_string_lossy()))
}

/// Where `kind`'s report of the scene at `scene_path` lives.
pub fn report_path(scene_path: &str, kind: Kind) -> PathBuf {
    report_dir(scene_path).join(format!("{}.json", kind.name()))
}

/// One bake's report: the common block, then the kind's own fields.
#[derive(Debug)]
pub struct Report {
    kind: Kind,
    fields: Map<String, Value>,
}

impl Report {
    /// The common block of a `kind` bake that ran `stages` with `settings`, read
    /// inputs fingerprinted `fingerprint` and wrote `output_bytes`.
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

    /// Add the kind's own fields: `body` is an object, each of its keys a field.
    pub fn with(mut self, body: Value) -> Self {
        if let Value::Object(body) = body {
            self.fields.extend(body);
        }
        self
    }

    pub fn to_json(&self) -> Value {
        Value::Object(self.fields.clone())
    }

    /// Write the report beside the scene at `scene_path`, replacing the last one of
    /// its kind; returns the file written.
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

/// [`Report::write`], a failure only warned: a report is a diagnostic, and a bake
/// that finished keeps what it baked whatever happens to it.
pub fn keep(scene_path: &str, report: Report) {
    if let Err(e) = report.write(scene_path) {
        log::warn!(
            "[BakeReport] could not write the {} report: {e}",
            report.kind.name()
        );
    }
}

/// The last `kind` report of the scene at `scene_path`; `None` when there is none
/// or it does not parse.
pub fn read(scene_path: &str, kind: Kind) -> Option<Value> {
    let text = std::fs::read_to_string(report_path(scene_path, kind)).ok()?;
    serde_json::from_str(&text).ok()
}

/// Bytes of every file under `path` (a file or a folder); 0 when it is missing.
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

/// The CPU workers a threaded bake runs on: every core. The one definition the
/// occlusion bake starts with and the reports say.
pub fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

/// What a `kind` bake runs on, as its report's `threads` says it: the lightmap
/// tracer and the occlusion bake run on [`threads`] workers, the navmesh bake on
/// the calling thread (1), and the probe and reflection captures on the GPU
/// (`"gpu"`).
pub fn workers(kind: Kind) -> Value {
    match kind {
        Kind::Lightmaps | Kind::Occlusion => json!(threads()),
        Kind::Navmesh => json!(1),
        Kind::Probes | Kind::Reflections => json!("gpu"),
    }
}

/// Entity `id`'s centre as the renderer picks its reflection probe there: its
/// mesh's local bounds under its world matrix (`render::transform_aabb`, what
/// the renderer's world bounds are), halved. A rotated asymmetric mesh's centre
/// differs from the middle of its vertices' world box. A skinned mesh is taken
/// at rest. `None` without a mesh or with no vertex.
pub fn drawn_centre(scene: &rusty_scene::Scene, id: u32) -> Option<glam::Vec3> {
    let world = &scene.world;
    let (min, max) = world.mesh(id)?.world_aabb(glam::Mat4::IDENTITY)?;
    let (min, max) = rusty_render::transform_aabb(min, max, world.local_to_world(id));
    Some((min + max) * 0.5)
}

/// Seconds since the Unix epoch, 0 on a clock before it.
fn unix_seconds() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    now.map_or(0, |d| d.as_secs())
}

/// `seconds` rounded to the millisecond, so a report reads cleanly.
pub fn secs(d: std::time::Duration) -> f64 {
    (d.as_secs_f64() * 1000.0).round() / 1000.0
}

/// A world point as `[x, y, z]`, rounded to the centimetre.
pub fn point(p: glam::Vec3) -> Value {
    let r = |v: f32| ((v as f64) * 100.0).round() / 100.0;
    json!([r(p.x), r(p.y), r(p.z)])
}

/// The entity's name, or `#<id>` without one, as the bake log names it.
pub fn entity_name(scene: &rusty_scene::Scene, id: u32) -> String {
    match scene.world.name(id) {
        Some(n) => n.to_string(),
        None => format!("#{id}"),
    }
}
