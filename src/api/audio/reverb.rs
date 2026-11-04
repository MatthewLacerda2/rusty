//! src/api/audio/reverb.rs — the `Audio` reverb-zone read-back (#469).
//!
//! The reverb the listener heard at the last `LateUpdate`: the zones around the
//! active camera, blended. Zones themselves are authored through
//! `AudioReverbZone.*`.

use mlua::Table;

use super::super::reverb_zone::params_table;
use super::super::{put, Reg};
use crate::api::ApiCell;
use crate::audio::AudioMaestro;

/// `GetReverbState`.
pub(super) fn register_reverb(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "GetReverbState",
        lua.create_function(move |lua, ()| {
            let state = audio.get(lua)?.borrow().reverb_state();
            let t = params_table(lua, &state.params)?;
            t.set("weight", state.weight)?;
            t.set("zones", state.zones)?;
            Ok(t)
        }),
    )
}
