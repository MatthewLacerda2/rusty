//! src/api/lighting.rs — `Lighting` namespace: the one-button bakes.
//!
//! `Lighting.Bake()` is the "place well + bake right now" workflow (#246): in one call
//! it auto-places light probes and reflection probes from the static scene (unless a set
//! is already manually authored), then runs the existing multi-bounce light-probe bake
//! (#250, #285) and GGX reflection-probe bake (#252). It is the orchestration over the `Probe`
//! and `Reflection` namespaces — manual placement on those stays available; this is the
//! batteries-included path. `Lighting.BakeLightmaps()` / `ClearLightmaps()` (#438) bake
//! static geometry's lightmaps beside it, and `Lighting.Generate()` (#832) runs both
//! in order and writes the result, also from `dev`.
//!
//! The bakes are dev-only (they drive a headless GPU and write authoring artifacts),
//! so this module registers only the table: `dev` installs the verbs onto it through
//! [`super::extend`] (#737), beside the editor's Generate Lighting button that runs
//! the same orchestration. In a ship build the `Lighting` table is present but empty,
//! matching the stripped agentic layer.

use mlua::Lua;

use super::Reg;

/// Register the (empty) `Lighting` namespace table onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    lua.globals()
        .set("Lighting", table)
        .map_err(|e| e.to_string())
}
