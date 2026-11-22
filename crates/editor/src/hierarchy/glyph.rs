//! The row glyph per kind of GameObject, so a Hierarchy row says what the object
//! *is* before its name. Kept apart from `tree` for the size gate.

use egui_phosphor::regular as icon;

use crate::theme::Theme;
use rusty_scene::Scene;

/// How a hierarchy glyph is tinted, resolved against the theme when drawn.
#[derive(Clone, Copy)]
enum Tint {
    Muted,
    Accent,
    Warning,
    Teal,
}

type Has = fn(&rusty_ecs::World, u32) -> bool;

/// The row glyph per kind of GameObject, most telling component first: a
/// row says what the object *is* — camera, light, UI, audio, character — before
/// falling back to the plain mesh cube, and an empty object gets a dashed circle.
const KINDS: &[(Has, &str, Tint)] = &[
    (|w, id| w.has_camera(id), icon::VIDEO_CAMERA, Tint::Accent),
    (|w, id| w.has_light(id), icon::LIGHTBULB, Tint::Warning),
    (|w, id| w.has_canvas(id), icon::BROWSER, Tint::Teal),
    (
        |w, id| w.has_rect_transform(id),
        icon::FRAME_CORNERS,
        Tint::Teal,
    ),
    (|w, id| w.has_particles(id), icon::SPARKLE, Tint::Warning),
    (
        |w, id| w.has_audio(id) || w.has_reverb_zone(id),
        icon::SPEAKER_HIGH,
        Tint::Teal,
    ),
    (
        |w, id| w.has_character_controller(id) || w.has_animator(id),
        icon::PERSON_SIMPLE,
        Tint::Accent,
    ),
    (
        |w, id| w.has_nav_agent(id),
        icon::NAVIGATION_ARROW,
        Tint::Accent,
    ),
    (
        |w, id| w.has_nav_obstacle(id) || w.has_nav_modifier(id) || w.has_offmesh_link(id),
        icon::PATH,
        Tint::Muted,
    ),
    (
        |w, id| w.has_trail(id) || w.has_line(id),
        icon::LINE_SEGMENTS,
        Tint::Muted,
    ),
    (|w, id| w.has_mesh(id), icon::CUBE, Tint::Muted),
    (|w, id| w.has_collider(id), icon::BOUNDING_BOX, Tint::Muted),
    (|w, id| w.has_joint(id), icon::LINK, Tint::Muted),
];

/// The row's glyph and its colour. Skeleton bones read as bones whatever they carry.
pub(super) fn node_glyph(scene: &Scene, id: u32, t: Theme) -> (&'static str, egui::Color32) {
    let (glyph, tint) = if scene.is_bone(id) {
        (icon::BONE, Tint::Muted)
    } else {
        KINDS
            .iter()
            .find(|(has, _, _)| has(&scene.world, id))
            .map_or((icon::CIRCLE_DASHED, Tint::Muted), |&(_, g, tint)| {
                (g, tint)
            })
    };
    let color = match tint {
        Tint::Muted => t.text_secondary,
        Tint::Accent => t.accent,
        Tint::Warning => t.warning,
        Tint::Teal => t.teal,
    };
    (glyph, color)
}
