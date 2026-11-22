//! Declares a script-facing enum with its API spelling written once: `ALL`, `name()`,
//! `parse()` (any ASCII case), `names()` and `Display`. A variant may take aliases
//! (`Custom = "Custom" | "User"`); `name()` is the first. The enum must be `Copy` and
//! must not derive serde: the macro writes it, so stored files hold `name()` and read
//! through `parse()`.

#[macro_export]
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
            pub const ALL: [Self; [$(stringify!($Variant)),+].len()] = [$(Self::$Variant),+];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$Variant => $name,)+
                }
            }

            pub fn names() -> String {
                Self::ALL.map(Self::name).join(", ")
            }

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
            $crate::naming::named_enum::unique(&[$($name, $($alias,)*)+]),
            concat!("two names of `", stringify!($Name), "` match ignoring case"),
        );

        impl ::std::fmt::Display for $Name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.name())
            }
        }

        const _: () = {
            use $crate::__serde::de::{self, EnumAccess, VariantAccess};

            const NAMES: &[&str] = &[$($name),+];

            impl $crate::__serde::Serialize for $Name {
                fn serialize<S: $crate::__serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                    let index = Self::ALL.iter().position(|v| v == self).unwrap_or(0);
                    s.serialize_unit_variant(stringify!($Name), index as u32, self.name())
                }
            }

            struct Tag($Name);

            impl<'de> $crate::__serde::Deserialize<'de> for Tag {
                fn deserialize<D: $crate::__serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
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

            impl<'de> $crate::__serde::Deserialize<'de> for $Name {
                fn deserialize<D: $crate::__serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                    Ok(d.deserialize_enum(stringify!($Name), NAMES, Visitor)?.0)
                }
            }
        };
    };
}

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
