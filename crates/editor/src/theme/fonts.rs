//! Phosphor's icons sit in the Private Use Area, where Inter's files carry no glyphs;
//! Inter leads the family, so any it had would draw over them. Phosphor cannot lead instead: it
//! maps `a`–`z` to the blanks its ligatures are built from.

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

const INTER: &[u8] = include_bytes!("../../../../engine/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../../engine/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../../../engine/fonts/JetBrainsMono-Regular.ttf");

const SEMIBOLD: &str = "inter-semibold";

/// A [`FontId`] in Inter SemiBold at `size` points — the editor's "strong" weight.
/// The regular weight until the fonts load, a pass after `install`: egui panics on a
/// family it does not know.
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
    #[test]
    fn inter_leaves_the_icon_codepoints_to_phosphor() {
        let icon = egui_phosphor::regular::FOLDER.chars().next().unwrap();
        for bytes in [super::INTER, super::INTER_SEMIBOLD] {
            use skrifa::MetadataProvider;
            let charmap = skrifa::FontRef::new(bytes).unwrap().charmap();
            assert!(charmap.map('A').is_some());
            assert!(charmap.map(icon).is_none(), "Inter maps {icon:?}");
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
