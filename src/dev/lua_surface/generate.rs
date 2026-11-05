//! `Lighting.Generate()` and the scene's lighting settings (#832): the one verb
//! behind the editor's Generate Lighting button, and `Lighting.GetSettings()` /
//! `SetSettings{…}`, the knobs the button's card edits (the lightmap bake's, its
//! Max Lightmap Size included, #1704, and the
//! reflection probes' default resolution, memory budget and probe ceiling, #1584,
//! and the scene's diffuse model, #1698).

use mlua::{Lua, Table};

use super::super::bake::generate_lighting::generate_lighting;
use super::super::bake::progress::BakeLog;
use crate::api::{global_table, put, ApiCell, ApiCtx, Reg};
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::lighting::settings::ReflectionSettings;
use crate::scene::lighting::{DiffuseModel, MixedLightingMode};
use crate::scene::Scene;

/// The knobs a script may override, each optional; `None` keeps `base`'s.
#[derive(Default)]
pub(super) struct Overrides {
    pub texels_per_unit: Option<f32>,
    pub samples: Option<u32>,
    pub bounces: Option<u32>,
    pub seed: Option<u64>,
    pub directional: Option<bool>,
    /// Max Lightmap Size, the atlas page edge (#1704).
    pub max_lightmap_size: Option<u32>,
}

impl Overrides {
    /// `base` with these overrides applied, then clamped into
    /// [`BakeSettings::sanitized`]'s ranges, the editor's.
    pub(super) fn apply(&self, base: BakeSettings) -> BakeSettings {
        BakeSettings {
            texels_per_unit: self.texels_per_unit.unwrap_or(base.texels_per_unit),
            samples: self.samples.unwrap_or(base.samples),
            bounces: self.bounces.unwrap_or(base.bounces),
            seed: self.seed.unwrap_or(base.seed),
            directional: self.directional.unwrap_or(base.directional),
            max_lightmap_size: self.max_lightmap_size.unwrap_or(base.max_lightmap_size),
            ..base
        }
        .sanitized()
    }

    fn from_table(t: &Table) -> mlua::Result<Self> {
        Ok(Self {
            texels_per_unit: t.get("texelsPerUnit")?,
            samples: t.get("samples")?,
            bounces: t.get("bounces")?,
            seed: t.get("seed")?,
            directional: t.get("directional")?,
            max_lightmap_size: t.get("maxLightmapSize")?,
        })
    }
}

/// Add `Generate`, `GetSettings` and `SetSettings` to the `Lighting` table.
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
            let out = lua.create_table()?;
            out.set("lightmaps", report.lightmaps)?;
            out.set("lightProbes", report.probes.light_probes)?;
            out.set("reflectionProbes", report.probes.reflection_probes)?;
            out.set("saved", report.saved == crate::scene::LightingSave::Written)?;
            Ok(out)
        }),
    )?;
    register_settings(lua, &table, scene)
}

/// `GetSettings` / `SetSettings`: the bake settings `Generate` uses.
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
            out.set("bounces", s.bounces)?;
            out.set("seed", s.seed)?;
            out.set("directional", s.directional)?;
            out.set("maxLightmapSize", s.max_lightmap_size)?;
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

/// `SetSettings{…}`: the fields `t` names written into the scene's settings, each
/// clamped. Every field is read and checked before any is written, so a call that
/// raises (an unknown diffuse model, a value of the wrong type) changes nothing.
fn set_settings(scene: &mut crate::scene::Scene, t: &Table) -> mlua::Result<()> {
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

/// The diffuse model `t` names (#1698), if any; an unknown name is an error.
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

/// The Mixed Lighting Mode `t` names (#1695), if any; an unknown name is an error.
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

/// The reflection probes' defaults, as `GetSettings` returns them (#1584).
fn put_reflections(out: &Table, r: &ReflectionSettings) -> mlua::Result<()> {
    out.set("reflectionResolution", r.resolution)?;
    out.set("reflectionBudgetMB", r.budget_mb)?;
    out.set("reflectionMaxProbes", r.max_probes)
}

/// The reflection defaults `t` names written into `r`, then clamped.
fn set_reflections(t: &Table, r: &mut ReflectionSettings) -> mlua::Result<()> {
    let get = |key: &str| t.get::<Option<u32>>(key);
    r.resolution = get("reflectionResolution")?.unwrap_or(r.resolution);
    r.budget_mb = get("reflectionBudgetMB")?.unwrap_or(r.budget_mb);
    r.max_probes = get("reflectionMaxProbes")?.unwrap_or(r.max_probes);
    r.clamp();
    Ok(())
}

#[cfg(test)]
#[path = "generate_tests.rs"]
mod tests;
