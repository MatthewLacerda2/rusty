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
//! anything else), `names()` (every name, joined for an error's "one of …") and
//! `Display` (the name). A variant may take aliases its parse also accepts,
//! `Custom = "Custom" | "User"`; `name()` is always the first. Two spellings of one
//! enum that match ignoring case fail the build ([`unique`]), so `parse` is never
//! ambiguous.
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
            $crate::core::named_enum::unique(&[$($name, $($alias,)*)+]),
            concat!("two names of `", stringify!($Name), "` match ignoring case"),
        );

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.name())
            }
        }
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
