//! How a background bake ended, said twice (#1999): in the console, and as the
//! scene's `lighting_bake_status`, which `Lighting.GetBakeStatus()` reads, since
//! the console alone reaches no agent.

use super::Step;
use crate::scene::lighting::bake_status::BakeStatus;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Say the bake ended with `summary`, in the console and as the scene's status.
pub(super) fn done(
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    summary: String,
    needs_save: bool,
) -> bool {
    log.info(summary.clone());
    scene.lighting_bake_status = BakeStatus::Done(summary);
    needs_save
}

/// Say Bake Occlusion failed; `needs_save` when its PVS was applied, never written.
pub(super) fn fail_occlusion(
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    e: String,
    needs_save: bool,
) -> Option<bool> {
    failed(
        scene,
        log,
        format!("Bake Occlusion failed: {e}"),
        needs_save,
    )
}

/// Say the bake failed; `needs_save` when what it applied was never written.
pub(super) fn fail(
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    e: String,
    needs_save: bool,
) -> Option<bool> {
    failed(
        scene,
        log,
        format!("Generate Lighting failed: {e}"),
        needs_save,
    )
}

/// Say `line` in the console and as the scene's status, which a script polls
/// (`Lighting.GetBakeStatus`, #1999): the console alone reaches no agent.
pub(super) fn failed(
    scene: &mut Scene,
    log: &mut ConsoleLogs,
    line: String,
    needs_save: bool,
) -> Option<bool> {
    log.error(line.clone());
    scene.lighting_bake_status = BakeStatus::Failed(line);
    Some(needs_save)
}

/// Which button started the bake `step` belongs to.
pub(super) fn verb(step: &Step) -> &'static str {
    match step {
        Step::OcclusionOnly { .. } => "Bake Occlusion",
        _ => "Generate Lighting",
    }
}

/// The console line for a bake dropped because `why`, saying what reached the scene.
pub(super) fn dropped(step: &Step, why: &str) -> String {
    match step {
        Step::Lightmaps(_) => format!("Generate Lighting dropped: {why}; nothing was applied"),
        Step::Probes { .. } => format!(
            "Generate Lighting dropped: {why}; the lightmaps were applied, the probes kept their old lighting"
        ),
        Step::Occlusion { .. } => format!(
            "Generate Lighting dropped: {why}; the lightmaps and probes were applied and written, the occlusion culling kept its old bake"
        ),
        Step::OcclusionOnly { .. } => {
            format!("Bake Occlusion dropped: {why}; the occlusion culling kept its old bake")
        }
    }
}
