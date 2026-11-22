use crate::authoring::defaults::attach_default_material;
use crate::authoring::dependency;
use crate::authoring::fields::KINDS;
use rusty_components::registry::first_class;
use rusty_components::FirstClass;
use rusty_scene::Scene;

macro_rules! component_kinds {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum ComponentKind {
            $($kind),*
        }

        impl ComponentKind {
            pub const ALL: &'static [ComponentKind] = &[$(Self::$kind),*];

            pub fn parse(name: &str) -> Option<Self> {
                let lower = name.to_lowercase();
                $(
                    if lower.eq_ignore_ascii_case(stringify!($kind))
                        || lower.eq_ignore_ascii_case(<$ty as FirstClass>::NAME)
                        || <$ty as FirstClass>::ALIASES.contains(&lower.as_str())
                    {
                        return Some(Self::$kind);
                    }
                )*
                None
            }
        }
    };
}

first_class!(component_kinds);

impl ComponentKind {
    pub fn requires(self) -> &'static [ComponentKind] {
        match self {
            ComponentKind::PostProcessing => &[ComponentKind::Camera],
            ComponentKind::Image
            | ComponentKind::Text
            | ComponentKind::Shape
            | ComponentKind::RectMask
            | ComponentKind::Mask
            | ComponentKind::BackdropFilter
            | ComponentKind::Selectable
            | ComponentKind::LayoutGroup
            | ComponentKind::LayoutElement => &[ComponentKind::RectTransform],
            ComponentKind::Joint => &[ComponentKind::RigidBody],
            _ => &[],
        }
    }

    pub fn parse_or_list(name: &str) -> Result<Self, String> {
        Self::parse(name).ok_or_else(|| {
            let kinds: Vec<&str> = KINDS
                .iter()
                .filter(|k| k.add.is_some())
                .map(|k| k.name)
                .collect();
            format!("unknown component '{name}' (kinds: {})", kinds.join(", "))
        })
    }
}

pub fn add_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    if kind == ComponentKind::Texture {
        return attach_default_material(scene, id);
    }
    let added = dependency::add_with_requirements(&mut scene.world, id, kind);
    if added && kind == ComponentKind::Light {
        rusty_scene::create::live_in_play(scene, id);
    }
    added
}

pub fn remove_component(scene: &mut Scene, id: u32, kind: ComponentKind) -> bool {
    dependency::remove_with_cascade(&mut scene.world, id, kind)
}

#[cfg(test)]
#[path = "components_tests.rs"]
mod tests;
