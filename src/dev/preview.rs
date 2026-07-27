//! src/dev/preview.rs — Headless asset preview → PNG (the agent's eyes on assets, #353).
//!
//! The agent-facing half of the #352 preview scene. The editor's Preview tab lets a
//! *human* see how a model/texture/shader looks before placing it; this renders the
//! **same isolated scene** ([`crate::preview`]) with no window and writes a `.png`, so
//! an agent can close the authoring loop it currently can't: **bake → preview → look →
//! iterate**. `Shader.Bake` proves a `.wgsl` compiles; only this shows what it renders.
//!
//! Framing is **fixed** ([`OrbitState::default`]) rather than parameterised: the orbit
//! drag is the human affordance, while an agent needs two shots of the same asset to be
//! diffable pixel-for-pixel. Same reason the resolution defaults to a constant.
//!
//! Shares every moving part it can — [`crate::preview::build_preview_scene`] for the
//! scene, `Renderer::render_preview_with_shader` for the `.wgsl` arm, and
//! `render::readback` for the copy-back + encode — so there is exactly one preview
//! machinery, not an editor one and an agent one that drift.
//!
//! Runtime caveat: like `screenshot`, this needs a GPU or software adapter (e.g.
//! lavapipe). With none available it returns `Ok(false)` after a clear log and NEVER
//! panics, so a headless CI box without a GPU degrades instead of failing.
//!
//! Allowed deps: preview, render (headless path).

use std::path::Path;

use super::capture::CaptureHost;
use crate::preview::{self, OrbitState, PreviewMesh, PreviewSubject};

/// Default edge length of a preview shot. Square, like every reference engine's
/// material-preview thumbnail, and big enough to judge shading detail.
pub const DEFAULT_RESOLUTION: u32 = 512;
/// Bounds on a requested resolution, so a typo'd `resolution = 100000` is clamped
/// rather than trying to allocate a multi-gigabyte target.
const MIN_RESOLUTION: u32 = 16;
const MAX_RESOLUTION: u32 = 4096;

/// The knobs a caller may turn. Deliberately small: the camera is *not* here, because
/// deterministic framing is the point (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewOptions {
    /// Which stand-in mesh a texture/shader/material is shown on. Ignored for models,
    /// which are rendered as themselves.
    pub mesh: PreviewMesh,
    /// Edge length in pixels of the square output, clamped to a sane range.
    pub resolution: u32,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            mesh: PreviewMesh::Sphere,
            resolution: DEFAULT_RESOLUTION,
        }
    }
}

impl PreviewOptions {
    /// The resolution actually rendered: clamped into `[MIN_RESOLUTION, MAX_RESOLUTION]`.
    fn clamped_resolution(self) -> u32 {
        self.resolution.clamp(MIN_RESOLUTION, MAX_RESOLUTION)
    }
}

/// Render `asset_path`'s preview into a PNG at `out_png`, through `host`'s shared
/// renderer — so previewing a folder of assets costs one device, not one per asset
/// (#355 step 5).
///
/// Returns `Ok(true)` when a frame was captured and written, `Ok(false)` when no
/// GPU/software adapter is available (skipped gracefully, matching `screenshot`), and
/// `Err` for a caller mistake — an asset that doesn't exist or has no preview story —
/// so an agent gets a clear message instead of a plausible picture of nothing.
pub fn capture_asset_into(
    host: &mut CaptureHost,
    asset_path: &str,
    out_png: impl AsRef<Path>,
    options: PreviewOptions,
) -> Result<bool, String> {
    let subject = resolve_subject(asset_path)?;
    let scene = preview::build_preview_scene(options.mesh, &subject);
    let resolution = options.clamped_resolution();

    let Some((renderer, view)) = host.frame(resolution, resolution) else {
        log::warn!(
            "[Preview] no GPU/software adapter available — skipping preview of {asset_path}"
        );
        return Ok(false);
    };
    let Some(target_view) = view.color_target_view() else {
        return Err("preview view owns no colour target".to_string());
    };
    let camera = OrbitState::default().camera();

    // The shader arm draws with the previewed module as this view's forward pipeline —
    // the same call the editor tab makes; every other subject renders the stock one.
    match &subject {
        PreviewSubject::Shader(path) => {
            renderer.render_preview_with_shader(view, &scene, &camera, &target_view, path);
        }
        _ => renderer.render(view, &scene, &camera, &target_view, false, &[]),
    }

    host.write_png(out_png)?;
    Ok(true)
}

/// One-shot [`capture_asset_into`]: a throwaway host for a single preview. Prefer
/// holding a host when previewing more than one asset.
pub fn capture_asset(
    asset_path: &str,
    out_png: impl AsRef<Path>,
    options: PreviewOptions,
) -> Result<bool, String> {
    capture_asset_into(&mut CaptureHost::new(), asset_path, out_png, options)
}

/// The preview subject for `asset_path`, or a caller-facing error explaining exactly
/// which of the two things went wrong. Checking existence up front matters because
/// [`preview::build_preview_scene`] is deliberately forgiving — a failed import leaves
/// an empty scene, which would render a perfectly nice picture of empty sky and let a
/// typo'd path pass for a working preview.
fn resolve_subject(asset_path: &str) -> Result<PreviewSubject, String> {
    if !Path::new(asset_path).is_file() {
        return Err(format!("no such asset: {asset_path}"));
    }
    preview::subject_for_path(asset_path).ok_or_else(|| {
        format!(
            "no preview for {asset_path} — previewable kinds are models \
             (gltf/glb/obj/fbx), textures (png/tga/jpg/jpeg) and shaders (wgsl)"
        )
    })
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod preview_tests;
