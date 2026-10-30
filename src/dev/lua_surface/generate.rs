//! `Lighting.Generate()` and the scene's lighting settings (#832): the one verb
//! behind the editor's Generate Lighting button, and `Lighting.GetSettings()` /
//! `SetSettings{…}`, the knobs the button's card edits (the lightmap bake's, and the
//! reflection probes' default resolution, memory budget and probe ceiling, #1584).

use mlua::{Lua, Table};

use super::super::bake::generate_lighting::generate_lighting;
use crate::api::{global_table, put, ApiScopedCtx, Reg};
use crate::scene::lighting::lightmap::BakeSettings;
use crate::scene::lighting::settings::ReflectionSettings;

/// The knobs a script may override, each optional; `None` keeps `base`'s.
#[derive(Default)]
pub(super) struct Overrides {
    pub texels_per_unit: Option<f32>,
    pub samples: Option<u32>,
    pub bounces: Option<u32>,
    pub seed: Option<u64>,
    pub directional: Option<bool>,
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
        })
    }
}

/// Add `Generate`, `GetSettings` and `SetSettings` to the `Lighting` table.
pub(super) fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let (scene, scene_path, nav) = (ctx.scene, ctx.scene_path, ctx.nav);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "Generate",
        scope.create_function(move |lua, ()| {
            let path = scene_path.borrow();
            let nav = nav.borrow();
            let mut scene = scene.borrow_mut();
            let report = generate_lighting(&mut scene, path.as_deref(), Some(&nav))
                .map_err(mlua::Error::external)?;
            let out = lua.create_table()?;
            out.set("lightmaps", report.lightmaps)?;
            out.set("lightProbes", report.probes.light_probes)?;
            out.set("reflectionProbes", report.probes.reflection_probes)?;
            out.set("saved", report.saved == crate::scene::LightingSave::Written)?;
            Ok(out)
        }),
    )?;
    put(
        &table,
        "GetSettings",
        scope.create_function(move |lua, ()| {
            let settings = scene.borrow().lighting_settings;
            let (s, r) = (settings.lightmaps, settings.reflections);
            let out = lua.create_table()?;
            out.set("texelsPerUnit", s.texels_per_unit)?;
            out.set("samples", s.samples)?;
            out.set("bounces", s.bounces)?;
            out.set("seed", s.seed)?;
            out.set("directional", s.directional)?;
            put_reflections(&out, &r)?;
            Ok(out)
        }),
    )?;
    put(
        &table,
        "SetSettings",
        scope.create_function(move |_, t: Table| {
            let overrides = Overrides::from_table(&t)?;
            let mut scene = scene.borrow_mut();
            let s = &mut scene.lighting_settings.lightmaps;
            *s = overrides.apply(*s);
            set_reflections(&t, &mut scene.lighting_settings.reflections)
        }),
    )
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
