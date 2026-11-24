use std::cell::RefCell;

use mlua::{Lua, Table};

use super::super::bake::generate_lighting::{bake_occlusion_only, generate_lighting};
use super::super::bake::occlusion::unsaved_materials_warning;
use super::super::bake::progress::BakeLog;
use rusty_scene::lighting::settings::ReflectionSettings;
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};
use rusty_scene::lighting::{DiffuseModel, MixedLightingMode};
use rusty_scene::Scene;
use rusty_script::api::{global_table, put, ApiCell, ApiCtx, Reg};
use rusty_script::scripting::ConsoleLogs;

#[derive(Default)]
pub(super) struct Overrides {
    pub texels_per_unit: Option<f32>,
    pub samples: Option<u32>,
    pub direct_samples: Option<u32>,
    pub bounces: Option<u32>,
    pub seed: Option<u64>,
    pub directional: Option<bool>,
    pub max_lightmap_size: Option<u32>,
    pub denoiser: Option<Denoiser>,
    pub backface_tolerance: Option<f32>,
    pub samples_per_pass: Option<u32>,
    pub prioritize_view: Option<bool>,
}

impl Overrides {
    pub(super) fn apply(&self, base: BakeSettings) -> BakeSettings {
        BakeSettings {
            texels_per_unit: self.texels_per_unit.unwrap_or(base.texels_per_unit),
            samples: self.samples.unwrap_or(base.samples),
            direct_samples: self.direct_samples.unwrap_or(base.direct_samples),
            bounces: self.bounces.unwrap_or(base.bounces),
            seed: self.seed.unwrap_or(base.seed),
            directional: self.directional.unwrap_or(base.directional),
            max_lightmap_size: self.max_lightmap_size.unwrap_or(base.max_lightmap_size),
            denoiser: self.denoiser.unwrap_or(base.denoiser),
            backface_tolerance: self.backface_tolerance.unwrap_or(base.backface_tolerance),
            samples_per_pass: self.samples_per_pass.unwrap_or(base.samples_per_pass),
            prioritize_view: self.prioritize_view.unwrap_or(base.prioritize_view),
            ..base
        }
        .sanitized()
    }

    fn from_table(t: &Table) -> mlua::Result<Self> {
        Ok(Self {
            texels_per_unit: t.get("texelsPerUnit")?,
            samples: t.get("samples")?,
            direct_samples: t.get("directSamples")?,
            bounces: t.get("bounces")?,
            seed: t.get("seed")?,
            directional: t.get("directional")?,
            max_lightmap_size: t.get("maxLightmapSize")?,
            denoiser: denoiser(t)?,
            backface_tolerance: t.get("backfaceTolerance")?,
            samples_per_pass: t.get("samplesPerPass")?,
            prioritize_view: t.get("prioritizeView")?,
        })
    }
}

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, nav, console) = (ctx.scene, ctx.scene_path, ctx.nav, ctx.console);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "Generate",
        lua.create_function(move |lua, ()| {
            let (scene, scene_path) = (scene.get(lua)?, scene_path.get(lua)?);
            let (nav, console) = (nav.get(lua)?, console.get(lua)?);
            let (path, nav) = (scene_path.borrow(), nav.borrow());
            let mut scene = scene.borrow_mut();
            let mut say = |line| console.borrow_mut().info(line);
            let mut log = BakeLog::to_console(&console);
            let (path, nav) = (path.as_deref(), Some(&*nav));
            let report = generate_lighting(&mut scene, path, nav, &mut say, &mut log)
                .map_err(mlua::Error::external)?;
            warn_unsaved(&console, &scene, path);
            let out = lua.create_table()?;
            out.set("lightmaps", report.lightmaps)?;
            out.set("lightProbes", report.probes.light_probes)?;
            out.set("reflectionProbes", report.probes.reflection_probes)?;
            out.set("occlusionCells", report.occlusion.cells)?;
            out.set("occlusionObjects", report.occlusion.objects)?;
            out.set("occlusionBytes", report.occlusion.bytes)?;
            out.set("occlusionNeverCulled", report.occlusion.never_culled)?;
            out.set("saved", report.saved == rusty_edit::LightingSave::Written)?;
            Ok(out)
        }),
    )?;
    register_bake_occlusion(lua, &table, ctx)?;
    background::register(lua, ctx)?;
    queue::register(lua, ctx)?;
    register_settings(lua, &table, scene)
}

fn register_bake_occlusion(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, nav, console) = (ctx.scene, ctx.scene_path, ctx.nav, ctx.console);
    put(
        table,
        "BakeOcclusion",
        lua.create_function(move |lua, ()| {
            let (scene, scene_path) = (scene.get(lua)?, scene_path.get(lua)?);
            let (nav, console) = (nav.get(lua)?, console.get(lua)?);
            let (path, nav) = (scene_path.borrow(), nav.borrow());
            let mut log = BakeLog::to_console(&console);
            let mut scene = scene.borrow_mut();
            let (path, nav) = (path.as_deref(), Some(&*nav));
            let report = bake_occlusion_only(&mut scene, path, nav, &mut log)
                .map_err(mlua::Error::external)?;
            console.borrow_mut().info(report.summary());
            warn_unsaved(&console, &scene, path);
            let (o, out) = (report.occlusion, lua.create_table()?);
            out.set("cells", o.cells)?;
            out.set("objects", o.objects)?;
            out.set("occluders", o.occluders)?;
            out.set("neverCulled", o.never_culled)?;
            out.set("bytes", o.bytes)?;
            out.set("tooLarge", o.too_large)?;
            Ok(out)
        }),
    )
}

fn warn_unsaved(console: &RefCell<ConsoleLogs>, scene: &Scene, path: Option<&str>) {
    if let Some(line) = path.and_then(|p| unsaved_materials_warning(scene, p)) {
        eprintln!("{line}");
        console.borrow_mut().warn(line);
    }
}

fn register_settings(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetSettings",
        lua.create_function(move |lua, ()| {
            let settings = scene.get(lua)?.borrow().lighting_settings;
            let (s, r) = (settings.lightmaps, settings.reflections);
            let out = lua.create_table()?;
            out.set("texelsPerUnit", s.texels_per_unit)?;
            out.set("samples", s.samples)?;
            out.set("directSamples", s.direct_samples)?;
            out.set("bounces", s.bounces)?;
            out.set("seed", s.seed)?;
            out.set("directional", s.directional)?;
            out.set("maxLightmapSize", s.max_lightmap_size)?;
            out.set("denoiser", s.denoiser.name())?;
            out.set("backfaceTolerance", s.backface_tolerance)?;
            out.set("samplesPerPass", s.samples_per_pass)?;
            out.set("prioritizeView", s.prioritize_view)?;
            out.set("diffuseModel", settings.diffuse_model.name())?;
            out.set("mixedLightingMode", settings.mixed_lighting_mode.name())?;
            put_reflections(&out, &r)?;
            Ok(out)
        }),
    )?;
    put(
        table,
        "SetSettings",
        lua.create_function(move |lua, t: Table| {
            let scene = scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            set_settings(&mut scene, &t)
        }),
    )
}

/// Reads and checks every field before writing any, so a call that raises changes nothing.
fn set_settings(scene: &mut rusty_scene::Scene, t: &Table) -> mlua::Result<()> {
    let overrides = Overrides::from_table(t)?;
    let model = diffuse_model(t)?;
    let mixed = mixed_lighting_mode(t)?;
    let mut reflections = scene.lighting_settings.reflections;
    set_reflections(t, &mut reflections)?;
    let settings = &mut scene.lighting_settings;
    settings.diffuse_model = model.unwrap_or(settings.diffuse_model);
    settings.mixed_lighting_mode = mixed.unwrap_or(settings.mixed_lighting_mode);
    settings.lightmaps = overrides.apply(settings.lightmaps);
    settings.reflections = reflections;
    Ok(())
}

fn denoiser(t: &Table) -> mlua::Result<Option<Denoiser>> {
    let Some(name) = t.get::<Option<String>>("denoiser")? else {
        return Ok(None);
    };
    Denoiser::from_name(&name).map(Some).ok_or_else(|| {
        mlua::Error::runtime(format!("unknown denoiser '{name}' (OIDN, Filter or None)"))
    })
}

fn diffuse_model(t: &Table) -> mlua::Result<Option<DiffuseModel>> {
    let Some(name) = t.get::<Option<String>>("diffuseModel")? else {
        return Ok(None);
    };
    DiffuseModel::from_name(&name).map(Some).ok_or_else(|| {
        mlua::Error::runtime(format!(
            "unknown diffuseModel '{name}' (Lambert or HalfLambert)"
        ))
    })
}

fn mixed_lighting_mode(t: &Table) -> mlua::Result<Option<MixedLightingMode>> {
    let Some(name) = t.get::<Option<String>>("mixedLightingMode")? else {
        return Ok(None);
    };
    MixedLightingMode::parse(&name).map(Some).ok_or_else(|| {
        mlua::Error::runtime(format!(
            "unknown mixedLightingMode '{name}' ({})",
            MixedLightingMode::names()
        ))
    })
}

fn put_reflections(out: &Table, r: &ReflectionSettings) -> mlua::Result<()> {
    out.set("reflectionResolution", r.resolution)?;
    out.set("reflectionBudgetMB", r.budget_mb)?;
    out.set("reflectionMaxProbes", r.max_probes)
}

fn set_reflections(t: &Table, r: &mut ReflectionSettings) -> mlua::Result<()> {
    let get = |key: &str| t.get::<Option<u32>>(key);
    r.resolution = get("reflectionResolution")?.unwrap_or(r.resolution);
    r.budget_mb = get("reflectionBudgetMB")?.unwrap_or(r.budget_mb);
    r.max_probes = get("reflectionMaxProbes")?.unwrap_or(r.max_probes);
    r.clamp();
    Ok(())
}

mod background;
mod queue;

#[cfg(test)]
mod denoiser_tests;
#[cfg(test)]
mod tests;
