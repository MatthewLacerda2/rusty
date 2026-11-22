use std::collections::BTreeMap;
use std::path::Path;

use super::capture::CaptureHost;
use rusty_components::MaterialAsset;
use rusty_preview::{PreviewMesh, PreviewSubject};

pub const DEFAULT_RESOLUTION: u32 = 512;
const MIN_RESOLUTION: u32 = 16;
const MAX_RESOLUTION: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewOptions {
    pub mesh: PreviewMesh,
    pub resolution: u32,
    pub repeats: Option<u32>,
}

impl Default for PreviewOptions {
    fn default() -> Self {
        Self {
            mesh: PreviewMesh::Sphere,
            resolution: DEFAULT_RESOLUTION,
            repeats: None,
        }
    }
}

impl PreviewOptions {
    fn clamped_resolution(self) -> u32 {
        self.resolution.clamp(MIN_RESOLUTION, MAX_RESOLUTION)
    }
}

pub fn capture_asset_into(
    host: &mut CaptureHost,
    asset_path: &str,
    out_png: impl AsRef<Path>,
    options: PreviewOptions,
) -> Result<bool, String> {
    let subject = resolve_subject(asset_path)?;
    capture_subject_into(host, &subject, asset_path, out_png.as_ref(), options)
}

pub fn capture_material(
    materials: &BTreeMap<String, MaterialAsset>,
    name: &str,
    out_png: impl AsRef<Path>,
    options: PreviewOptions,
) -> Result<bool, String> {
    let material = materials
        .get(name)
        .ok_or_else(|| format!("no material asset named {name:?} in the scene's library"))?;
    let subject = PreviewSubject::Material(Box::new(material.clone()));
    let label = format!("material {name:?}");
    capture_subject_into(
        &mut CaptureHost::new(),
        &subject,
        &label,
        out_png.as_ref(),
        options,
    )
}

fn capture_subject_into(
    host: &mut CaptureHost,
    subject: &PreviewSubject,
    label: &str,
    out_png: &Path,
    options: PreviewOptions,
) -> Result<bool, String> {
    let scene = rusty_preview::build_preview_scene(options.mesh, options.repeats, subject)?;
    let resolution = options.clamped_resolution();

    let Some((renderer, view)) = host.frame(resolution, resolution, Default::default()) else {
        super::capture::no_adapter(&format!("preview {label}"))?;
        return Ok(false);
    };
    let Some(target_view) = view.color_target_view() else {
        return Err("preview view owns no colour target".to_string());
    };
    let camera = rusty_preview::framing(options.mesh, subject).camera();

    match subject {
        PreviewSubject::Shader(path) => {
            renderer.render_preview_with_shader(view, &scene, &camera, &target_view, path);
        }
        _ => renderer.render(view, &scene, &camera, &target_view, false),
    }

    host.write_png(out_png)?;
    Ok(true)
}

pub fn capture_asset(
    asset_path: &str,
    out_png: impl AsRef<Path>,
    options: PreviewOptions,
) -> Result<bool, String> {
    capture_asset_into(&mut CaptureHost::new(), asset_path, out_png, options)
}

fn resolve_subject(asset_path: &str) -> Result<PreviewSubject, String> {
    if !Path::new(asset_path).is_file()
        && !rusty_asset::convert::cached_without_source(Path::new(asset_path))
    {
        return Err(format!("no such asset: {asset_path}"));
    }
    rusty_preview::subject_for_path(asset_path).ok_or_else(|| {
        format!(
            "no preview for {asset_path} — previewable kinds are models \
             ({}), textures ({}) and shaders (wgsl)",
            rusty_asset::MODEL_EXTENSIONS.join("/"),
            rusty_asset::IMAGE_EXTENSIONS.join("/"),
        )
    })
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod preview_tests;

#[cfg(test)]
#[path = "preview_plane_tests.rs"]
mod preview_plane_tests;
