//! `named_enum!`: a script-facing enum and its API spelling, written once (#1329).
//!
//! An enum a script, the inspector or a stored setting names by string declares
//! each variant's name beside it:
//!
//! ```ignore
//! crate::core::named_enum! {
//!     /// How a joint constrains its bodies.
//!     #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
//! anything else) and `Display` (the name). A variant may take aliases its parse
//! also accepts, `Custom = "Custom" | "User"`; `name()` is always the first.
//!
//! The enum must be `Copy` (`name` takes `self`). Its serde derive is its own: a
//! stored file holds serde's spelling, read exactly, and a Lua table reaches it
//! through `api::lua_json::decode`, which reads every variant name in any case
//! (#1216). Hand-writing the trio instead is the `named-enum` lint finding.

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

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.name())
            }
        }
    };
}

pub(crate) use named_enum;

#[cfg(test)]
#[path = "named_enum_tests.rs"]
mod tests;
