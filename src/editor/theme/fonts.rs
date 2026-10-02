//! The editor's typefaces (#668): Inter for the UI, JetBrains Mono for the console
//! and code, Phosphor for icons. All three are embedded, so the editor looks the
//! same on every machine. Licences sit next to the files in `assets/fonts/`.

use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};

const INTER: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

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
        fonts
            .font_data
            .insert(name.into(), FontData::from_static(bytes));
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
