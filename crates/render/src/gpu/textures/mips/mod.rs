mod blit;

pub(crate) use blit::MipGenerator;

use rusty_asset::sidecar::TextureSettings;

pub(crate) use rusty_asset::texture_import::TextureKind;

/// Colour averages through the sRGB view, in linear light: averaging sRGB bytes darkens mips.
pub(crate) fn view_format(kind: TextureKind) -> wgpu::TextureFormat {
    match kind.is_colour() {
        true => super::IMAGE_FORMAT,
        false => super::DATA_FORMAT,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MipSettings {
    pub generate: bool,
    pub kind: TextureKind,
}

impl MipSettings {
    pub(crate) fn for_map(import: &TextureSettings, kind: TextureKind) -> Self {
        Self {
            generate: import.generate_mipmaps,
            kind: kind.reading(import),
        }
    }

    pub(crate) fn levels(self, width: u32, height: u32) -> u32 {
        match self.generate {
            true => full_chain_len(width, height),
            false => 1,
        }
    }
}

pub(crate) use rusty_asset::texture_import::full_chain_len;

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
