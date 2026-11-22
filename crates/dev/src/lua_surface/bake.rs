use mlua::Lua;

use super::super::{
    bake::generate_lighting::refuse_while_baking, bake::lighting as lighting_bake,
    bake::lightmap as lightmap_bake, bake::navmesh::bake_navmesh, bake::probe as probe_bake,
    bake::progress::BakeLog, bake::reflection as reflection_bake,
};
use super::generate::Overrides;
use rusty_lighting::placement::MIN_REGION;
use rusty_script::api::{global_table, put, ApiCtx, Reg};

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    register_probe(lua, ctx)?;
    register_reflection(lua, ctx)?;
    register_lighting(lua, ctx)?;
    register_lightmaps(lua, ctx)?;
    register_navigation(lua, ctx)
}

fn register_navigation(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, nav, console) = (ctx.scene, ctx.scene_path, ctx.nav, ctx.console);
    put(
        &global_table(lua, "Navigation")?,
        "Bake",
        lua.create_function(move |lua, ()| {
            let scene = &*scene.get(lua)?;
            let path = scene_path.get(lua)?.borrow().clone();
            let nav = &*nav.get(lua)?;
            let mut scene = scene.borrow_mut();
            let written = bake_navmesh(&mut scene, &mut nav.borrow_mut(), path.as_deref())
                .map_err(mlua::Error::external)?;
            if let Some(file) = &written {
                console
                    .get(lua)?
                    .borrow_mut()
                    .info(format!("[Navigation] wrote {file}"));
            }
            Ok(written.is_some())
        }),
    )
}

fn register_probe(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, console) = (ctx.scene, ctx.scene_path, ctx.console);
    put(
        &global_table(lua, "Probe")?,
        "Bake",
        lua.create_function(move |lua, ()| {
            let scene = &*scene.get(lua)?;
            let path = scene_path.get(lua)?.borrow().clone();
            let mut scene = scene.borrow_mut();
            refuse_while_baking(&scene).map_err(mlua::Error::external)?;
            let console = &*console.get(lua)?;
            shadowmask_note(&scene, console, false);
            let mut log = BakeLog::to_console(console);
            probe_bake::bake_scene_probes_at(&mut scene, path.as_deref(), &mut log)
                .map_err(mlua::Error::external)
        }),
    )
}

fn shadowmask_note(
    scene: &rusty_scene::Scene,
    console: &std::cell::RefCell<rusty_script::scripting::ConsoleLogs>,
    lightmaps: bool,
) {
    if let Some(note) = scene.shadowmask_note(lightmaps) {
        console.borrow_mut().warn(note.to_string());
    }
}

fn register_reflection(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, console) = (ctx.scene, ctx.scene_path, ctx.console);
    put(
        &global_table(lua, "Reflection")?,
        "Bake",
        lua.create_function(move |lua, ()| {
            let scene = &*scene.get(lua)?;
            let scene_path = &*scene_path.get(lua)?;
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            refuse_while_baking(&scene).map_err(mlua::Error::external)?;
            let console = &*console.get(lua)?;
            let mut log = BakeLog::to_console(console);
            reflection_bake::bake_scene_reflections(&mut scene, path.as_deref(), &mut log)
                .map_err(mlua::Error::external)
        }),
    )
}

fn register_lighting(lua: &Lua, ctx: &ApiCtx) -> Reg {
    type BakeArgs = (Option<f32>, Option<f32>);
    let (scene, scene_path, nav, console) = (ctx.scene, ctx.scene_path, ctx.nav, ctx.console);
    put(
        &global_table(lua, "Lighting")?,
        "Bake",
        lua.create_function(move |lua, (spacing, region): BakeArgs| {
            let scene = &*scene.get(lua)?;
            let scene_path = &*scene_path.get(lua)?;
            let nav = &*nav.get(lua)?;
            let console = &*console.get(lua)?;
            let mut scene = scene.borrow_mut();
            let path = scene_path.borrow();
            let nav = nav.borrow();
            let params = bake_params(spacing, region)?;
            let mut say = |line| console.borrow_mut().info(line);
            let mut log = BakeLog::to_console(console);
            let report = lighting_bake::bake_lighting(
                &mut scene,
                path.as_deref(),
                Some(&nav),
                params,
                &mut say,
                &mut log,
            )
            .map_err(mlua::Error::external)?;
            Ok(report.ran_light_bake || report.ran_reflection_bake)
        }),
    )
}

fn bake_params(
    spacing: Option<f32>,
    region: Option<f32>,
) -> mlua::Result<lighting_bake::LightingBakeParams> {
    let mut params = lighting_bake::LightingBakeParams::default();
    params.probe_spacing = spacing.unwrap_or(params.probe_spacing);
    let region = region.unwrap_or(params.reflection.region);
    if region.is_nan() || region < MIN_REGION {
        return Err(mlua::Error::runtime(format!(
            "Lighting.Bake: reflectionRegion {region} is under the {MIN_REGION} m minimum"
        )));
    }
    params.reflection.region = region;
    Ok(params)
}

fn register_lightmaps(lua: &Lua, ctx: &ApiCtx) -> Reg {
    type Args = (
        Option<f32>,
        Option<u32>,
        Option<u32>,
        Option<u64>,
        Option<bool>,
    );
    let (scene, scene_path, console) = (ctx.scene, ctx.scene_path, ctx.console);
    let table = global_table(lua, "Lighting")?;
    put(
        &table,
        "BakeLightmaps",
        lua.create_function(
            move |lua, (texels_per_unit, samples, bounces, seed, directional): Args| {
                let scene = &*scene.get(lua)?;
                let scene_path = &*scene_path.get(lua)?;
                let overrides = Overrides {
                    texels_per_unit,
                    samples,
                    bounces,
                    seed,
                    directional,
                    ..Overrides::default()
                };
                let settings = overrides.apply(scene.borrow().lighting_settings.lightmaps);
                let path = scene_path.borrow();
                let mut scene = scene.borrow_mut();
                refuse_while_baking(&scene).map_err(mlua::Error::external)?;
                let console = &*console.get(lua)?;
                let mut log = BakeLog::to_console(console);
                let path = path.as_deref();
                let baked = lightmap_bake::bake_scene_lightmaps_logged(
                    &mut scene, path, &settings, &mut log,
                )
                .map_err(mlua::Error::external)?;
                log.finish();
                shadowmask_note(&scene, console, true);
                Ok(baked)
            },
        ),
    )?;
    put(
        &table,
        "ClearLightmaps",
        lua.create_function(move |lua, ()| {
            scene.get(lua)?.borrow_mut().lightmaps.clear();
            Ok(())
        }),
    )
}
