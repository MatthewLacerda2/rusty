//! `named_enum!`: a script-facing enum and its API spelling, written once (#1329).
//!
//! An enum a script, the inspector or a stored setting names by string declares
//! each variant's name beside it:
//!
//! ```ignore
//! crate::core::named_enum! {
//!     /// How a joint constrains its bodies.
//!     #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
//!     pub enum JointKind {
//!         #[default]
//!         Fixed = "Fixed",
//!         Hinge = "Hinge",
//!         Ball = "Ball",
//!     }
//! }
//! ```
//!
//! and gets the enum plus `ALL` (every variant, in declaration order), `name()`
//! (the spelling above), `parse()` (that spelling in any ASCII case, `None` for
//! anything else), `names()` (every name, joined for an error's "one of …") and
//! `Display` (the name). A variant may take aliases its parse also accepts,
//! `Custom = "Custom" | "User"`; `name()` is always the first. Two spellings of one
//! enum that match ignoring case fail the build ([`unique`]), so `parse` is never
//! ambiguous.
//!
//! The enum must be `Copy` (`name` takes `self`) and must not derive serde: the
//! macro writes `Serialize` and `Deserialize` too, so a stored file (a scene, a
//! prefab, a settings file) holds `name()` and is read through `parse()`, in any
//! case and with the aliases, the same names scripts, the inspector and the docs
//! use (#1478). A tag serde cannot read is reported as serde's own unknown variant,
//! listing the names. Hand-writing the trio instead is the `named-enum` lint finding.

/// See the module docs.
macro_rules! named_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $Name:ident {
            $(
                $(#[$vmeta:meta])*
                $Variant:ident = $name:literal $(| $alias:literal)*
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $Name {
            $( $(#[$vmeta])* $Variant, )+
        }

        impl $Name {
            /// Every variant, in declaration order.
            pub const ALL: [Self; [$(stringify!($Variant)),+].len()] = [$(Self::$Variant),+];

            /// The variant's name, as scripts, the inspector and the API docs spell it.
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$Variant => $name,)+
                }
            }

            /// Every variant's name, in declaration order, joined by `, ` (an error
            /// text's list of what it accepts).
            pub fn names() -> String {
                Self::ALL.map(Self::name).join(", ")
            }

            /// The variant `name` names, in any ASCII case (aliases too); `None` for
            /// an unknown name.
            pub fn parse(name: &str) -> Option<Self> {
                $(
                    if name.eq_ignore_ascii_case($name)
                        $(|| name.eq_ignore_ascii_case($alias))*
                    {
                        return Some(Self::$Variant);
                    }
                )+
                None
            }
        }

        const _: () = assert!(
            $crate::core::naming::named_enum::unique(&[$($name, $($alias,)*)+]),
            concat!("two names of `", stringify!($Name), "` match ignoring case"),
        );

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.name())
            }
        }

        // A stored file holds `name()` and is read through `parse()` (#1478).
        const _: () = {
            use ::serde::de::{self, EnumAccess, VariantAccess};

            const NAMES: &[&str] = &[$($name),+];

            impl ::serde::Serialize for $Name {
                fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    let index = Self::ALL.iter().position(|v| v == self).unwrap_or(0);
                    s.serialize_unit_variant(stringify!($Name), index as u32, self.name())
                }
            }

            /// The variant a stored name picks, read as serde's enum tag.
            struct Tag($Name);

            impl<'de> ::serde::Deserialize<'de> for Tag {
                fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    d.deserialize_identifier(Visitor)
                }
            }

            struct Visitor;

            impl<'de> de::Visitor<'de> for Visitor {
                type Value = Tag;

                fn expecting(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "one of {}", $Name::names())
                }

                fn visit_str<E: de::Error>(self, name: &str) -> Result<Tag, E> {
                    $Name::parse(name).map(Tag).ok_or_else(|| E::unknown_variant(name, NAMES))
                }

                fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Tag, A::Error> {
                    let (tag, rest) = data.variant::<Tag>()?;
                    rest.unit_variant()?;
                    Ok(tag)
                }
            }

            impl<'de> ::serde::Deserialize<'de> for $Name {
                fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    Ok(d.deserialize_enum(stringify!($Name), NAMES, Visitor)?.0)
                }
            }
        };
    };
}

pub(crate) use named_enum;

/// Whether no two of `names` are equal ignoring ASCII case: what keeps a
/// `named_enum!`'s `parse` unambiguous, checked when the enum compiles.
pub const fn unique(names: &[&str]) -> bool {
    let mut i = 0;
    while i < names.len() {
        let mut j = i + 1;
        while j < names.len() {
            if names[i].eq_ignore_ascii_case(names[j]) {
                return false;
            }
            j += 1;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
#[path = "named_enum_tests.rs"]
mod tests;
