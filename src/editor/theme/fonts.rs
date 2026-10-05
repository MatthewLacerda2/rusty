//! The editor's typefaces (#668): Inter for the UI, JetBrains Mono for the console
//! and code, Phosphor for icons. All three are embedded, so the editor looks the
//! same on every machine. Licences sit next to the files in `engine/fonts/`.
//!
//! The Inter files carry no Private Use Area codepoints (#333): Inter maps hundreds,
//! and since Phosphor 2.1 (egui-phosphor 0.6+) the icons sit in that same range, so
//! Inter, which leads the family, drew its own glyphs in place of the icons. Phosphor
//! cannot lead instead: it maps `a`–`z` to the blanks its ligatures are built from.

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

const INTER: &[u8] = include_bytes!("../../../engine/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../engine/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../../engine/fonts/JetBrainsMono-Regular.ttf");

/// The family name of Inter SemiBold, used for panel titles and card headers.
const SEMIBOLD: &str = "inter-semibold";

/// A [`FontId`] in Inter SemiBold at `size` points — the editor's "strong" weight.
/// (egui's `RichText::strong` only brightens the colour; this changes the weight.)
/// Fonts registered by [`install`] load on the *next* frame, and egui panics on a
/// family it does not know yet, so until then this is the regular weight.
pub fn semibold(ctx: &Context, size: f32) -> FontId {
    let family = FontFamily::Name(SEMIBOLD.into());
    let loaded = ctx.fonts(|f| f.families().contains(&family));
    FontId::new(
        size,
        if loaded {
            family
        } else {
            FontFamily::Proportional
        },
    )
}

/// Register Inter, JetBrains Mono and the Phosphor icon font. Call once.
pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    for (name, bytes) in [
        ("inter", INTER),
        (SEMIBOLD, INTER_SEMIBOLD),
        ("jetbrains-mono", JETBRAINS_MONO),
    ] {
        fonts.font_data.insert(
            name.into(),
            std::sync::Arc::new(FontData::from_static(bytes)),
        );
    }

    // Each family leads with its face; egui's defaults stay behind it as fallback
    // (emoji, the Phosphor glyphs, anything Inter does not cover).
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    let lead = |first: &str| [vec![first.to_string()], fallback.clone()].concat();
    fonts
        .families
        .insert(FontFamily::Proportional, lead("inter"));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), lead(SEMIBOLD));
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, "jetbrains-mono".into());
    mono.push("phosphor".into());
    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    /// An updated Inter that maps the Private Use Area again would hide every icon.
    #[test]
    fn inter_leaves_the_icon_codepoints_to_phosphor() {
        let icon = egui_phosphor::regular::FOLDER.chars().next().unwrap();
        for bytes in [super::INTER, super::INTER_SEMIBOLD] {
            let face = ttf_parser::Face::parse(bytes, 0).unwrap();
            assert!(face.glyph_index('A').is_some());
            assert!(face.glyph_index(icon).is_none(), "Inter maps {icon:?}");
        }
    }

    /// `install` registers the families the editor names; egui loads them a pass later.
    #[test]
    fn install_registers_the_semibold_and_icon_families() {
        let ctx = egui::Context::default();
        super::install(&ctx);
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let semibold = egui::FontFamily::Name(super::SEMIBOLD.into());
        assert_eq!(super::semibold(&ctx, 14.0).family, semibold);
        let icons = egui::FontFamily::Name("phosphor".into());
        assert!(ctx.fonts(|f| f.families().contains(&icons)));
    }
}
