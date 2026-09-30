//! Component discovery: first-class components are `Entity`'s `Option<…Component>`
//! fields, so a new component can't dodge the gates that enumerate them.

use super::axes::read;

const ENTITY: &str = "src/components/entity.rs";

/// Discover components from `Entity`'s `pub <field>: Option<…Component>` lines.
/// Shared with the parity gate (`parity.rs`), so both gates enumerate first-class
/// components from the same non-fragile source — `Entity`'s component fields.
pub(crate) fn discover() -> Vec<String> {
    discover_from(&read(ENTITY))
}

/// Pure core of [`discover`], over the `entity.rs` source text (so it is testable
/// without depending on the process working directory).
fn discover_from(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.trim().strip_prefix("pub ") else {
            continue;
        };
        let Some((field, ty)) = rest.split_once(": Option<") else {
            continue;
        };
        // `Option<…Component>,` — only optional component fields, never `parent_id`.
        let inner = ty.trim_end_matches([',', ' ']).trim_end_matches('>');
        if inner.ends_with("Component") {
            out.push(field.trim().to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_optional_component_fields_only() {
        let src = "\
            pub transform: TransformComponent,\n\
            pub camera: Option<CameraComponent>,\n\
            #[serde(default)]\n\
            pub particles: Option<ParticleEmitterComponent>,\n\
            pub parent_id: Option<u32>,\n";
        let fields = discover_from(src);
        assert!(fields.iter().any(|f| f == "particles"));
        assert!(fields.iter().any(|f| f == "camera"));
        // `transform` is mandatory (not Option) and `parent_id: Option<u32>` is not
        // a component — neither is discovered.
        assert!(!fields.iter().any(|f| f == "parent_id"));
        assert!(!fields.iter().any(|f| f == "transform"));
    }
}
