use std::path::PathBuf;

use mlua::{Lua, Table};
use rusty_asset::convert::texture_ref;

use super::runtime;
use rusty_asset::sidecar::{
    parse_max_size, Compression, FilterMode, TextureSettings, TextureType, WrapMode,
};

pub(super) fn file(path: &str) -> mlua::Result<PathBuf> {
    texture_ref::file(path).ok_or_else(|| {
        runtime(format!(
            "{path}: the model's import cache holds no such texture (import the model first)"
        ))
    })
}

pub(super) fn table(lua: &Lua, s: &TextureSettings) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.raw_set("wrapMode", s.wrap_mode.name())?;
    t.raw_set("filterMode", s.filter_mode.name())?;
    t.raw_set("generateMipmaps", s.generate_mipmaps)?;
    t.raw_set("textureType", s.texture_type.name())?;
    t.raw_set("sRGB", s.srgb)?;
    t.raw_set("compression", s.compression.name())?;
    t.raw_set("maxSize", s.max_size)?;
    t.raw_set("maxSizeChosen", s.max_size_chosen)?;
    t.raw_set("streamMipmaps", s.stream_mipmaps)?;
    Ok(t)
}

pub(super) fn settings(mut s: TextureSettings, t: &Table) -> mlua::Result<TextureSettings> {
    let stored_chosen = s.max_size_chosen;
    if let Some(name) = t.get::<Option<String>>("wrapMode")? {
        s.wrap_mode = WrapMode::parse(&name).map_err(runtime)?;
    }
    if let Some(name) = t.get::<Option<String>>("filterMode")? {
        s.filter_mode = FilterMode::parse(&name).map_err(runtime)?;
    }
    let mipmaps = t.get::<Option<bool>>("generateMipmaps")?;
    s.generate_mipmaps = mipmaps.unwrap_or(s.generate_mipmaps);
    if let Some(name) = t.get::<Option<String>>("textureType")? {
        s.texture_type = TextureType::parse(&name).map_err(runtime)?;
    }
    s.srgb = t.get::<Option<bool>>("sRGB")?.unwrap_or(s.srgb);
    let stream = t.get::<Option<bool>>("streamMipmaps")?;
    s.stream_mipmaps = stream.unwrap_or(s.stream_mipmaps);
    if let Some(name) = t.get::<Option<String>>("compression")? {
        s.compression = Compression::parse(&name).map_err(runtime)?;
    }
    if let Some(size) = t.get::<Option<u32>>("maxSize")? {
        let size = parse_max_size(size).map_err(runtime)?;
        if size != s.max_size {
            s = s.chosen(size);
        }
    }
    // Only a `maxSizeChosen` that differs from the stored flag says anything: the
    // one a Get table carries back must not undo a changed `maxSize`'s mark.
    if let Some(chosen) = t.get::<Option<bool>>("maxSizeChosen")? {
        if chosen != stored_chosen {
            s.max_size_chosen = chosen;
        }
    }
    Ok(s)
}

#[cfg(test)]
#[path = "texture_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "extracted_tests.rs"]
mod extracted_tests;
