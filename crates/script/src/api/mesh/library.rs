use mlua::{Lua, Table, Value};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::mesh::MeshData;
use rusty_scene::Scene;

pub fn register(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, (name, data): (String, Table)| {
        let data = mesh_data(&data).map_err(|e| refusal("Create", &e))?;
        let cell = scene.get(lua)?;
        let defined = cell.borrow_mut().define_mesh(&name, data);
        defined.map_err(|e| refusal("Create", &e))
    });
    put(t, "Create", f)?;
    let f = lua.create_function(move |lua, name: String| {
        let cell = scene.get(lua)?;
        let s = cell.borrow();
        match s.meshes.get(&name) {
            Some(mesh) => Ok(Value::Table(data_table(lua, &mesh.data)?)),
            None => Ok(Value::Nil),
        }
    });
    put(t, "GetAsset", f)?;
    let f = lua.create_function(move |lua, name: String| {
        Ok(scene.get(lua)?.borrow().meshes.contains_key(&name))
    });
    put(t, "HasAsset", f)?;
    let f = lua.create_function(move |lua, (name, opts): (String, Option<Table>)| {
        let force = match opts {
            Some(opts) => opts.get::<Option<bool>>("force")?.unwrap_or(false),
            None => false,
        };
        let cell = scene.get(lua)?;
        let deleted = cell.borrow_mut().delete_mesh(&name, force);
        deleted.map_err(|e| refusal("DeleteAsset", &e))
    });
    put(t, "DeleteAsset", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let cell = scene.get(lua)?;
        let s = cell.borrow();
        let mesh = s.world.mesh(id.read());
        Ok(mesh.map(|m| {
            m.asset_ref
                .clone()
                .unwrap_or_else(|| m.primitive_type.clone())
        }))
    });
    put(t, "GetSharedMesh", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live("Mesh", "SetSharedMesh")?;
        let cell = scene.get(lua)?;
        let set = rusty_edit::mesh::set_shared_mesh(&mut cell.borrow_mut(), id, &name);
        set.map_err(|e| refusal("SetSharedMesh", &e))
    });
    put(t, "SetSharedMesh", f)
}

fn refusal(verb: &str, why: &str) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Mesh.{verb}: {why}"))
}

/// Flat number arrays: `positions` in x,y,z triples, `uvs` in u,v pairs, `indices` 0-based.
fn mesh_data(t: &Table) -> Result<MeshData, String> {
    let numbers = |key: &str| -> Result<Vec<f64>, String> {
        let got: Option<Vec<f64>> = t.get(key).map_err(|e| format!("{key}: {e}"))?;
        Ok(got.unwrap_or_default())
    };
    let positions = triples("positions", &numbers("positions")?)?;
    let normals = triples("normals", &numbers("normals")?)?;
    let uvs = numbers("uvs")?;
    if !uvs.len().is_multiple_of(2) {
        return Err(format!(
            "{} uvs numbers is not a whole number of u,v pairs",
            uvs.len()
        ));
    }
    let uvs = uvs
        .chunks_exact(2)
        .map(|c| [c[0] as f32, c[1] as f32])
        .collect();
    let indices = numbers("indices")?;
    let index =
        |(at, &x): (usize, &f64)| match x.fract() == 0.0 && (0.0..=u32::MAX as f64).contains(&x) {
            true => Ok(x as u32),
            false => Err(format!("index {at} is {x}, not a whole number from 0")),
        };
    let indices = indices
        .iter()
        .enumerate()
        .map(index)
        .collect::<Result<_, _>>()?;
    Ok(MeshData {
        positions,
        indices,
        normals,
        uvs,
    })
}

fn triples(key: &str, values: &[f64]) -> Result<Vec<[f32; 3]>, String> {
    if !values.len().is_multiple_of(3) {
        return Err(format!(
            "{} {key} numbers is not a whole number of x,y,z triples",
            values.len()
        ));
    }
    Ok(values
        .chunks_exact(3)
        .map(|c| [c[0] as f32, c[1] as f32, c[2] as f32])
        .collect())
}

fn data_table(lua: &Lua, data: &MeshData) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set(
        "positions",
        data.positions
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<f32>>(),
    )?;
    t.set("indices", data.indices.clone())?;
    if !data.normals.is_empty() {
        t.set(
            "normals",
            data.normals.iter().flatten().copied().collect::<Vec<f32>>(),
        )?;
    }
    if !data.uvs.is_empty() {
        t.set(
            "uvs",
            data.uvs.iter().flatten().copied().collect::<Vec<f32>>(),
        )?;
    }
    Ok(t)
}

#[cfg(test)]
#[path = "library_tests.rs"]
mod tests;
