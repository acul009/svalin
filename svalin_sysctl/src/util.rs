/// Defines a string-serialized enum with optional input aliases and an
/// `Other(String)` fallback that preserves unrecognized values.
///
/// The first string for each variant is its canonical serialized value.
/// Serde traits need not be imported at the invocation site.
macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal $(| $alias:literal)*),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant,)*
            Other(::std::string::String),
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(match self {
                    $(Self::$variant => $value,)*
                    Self::Other(value) => value,
                })
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                Ok(match value.as_str() {
                    $($value $(| $alias)* => Self::$variant,)*
                    _ => Self::Other(value),
                })
            }
        }
    };
}

pub(crate) use string_enum;
