//! Open string enums: a fixed set of known values plus any other string, kept as written.
//!
//! Informational values (why a file was withheld, which input decided a file's sides, the
//! loader's or the server flavour's name) can gain new values without a schema change. A
//! reader that doesn't know a value keeps the text, so the image still serializes to the
//! same bytes and its id still verifies. Equality, order and hashing go by the text, so a
//! known value always equals itself however it was made.

/// Defines an open string enum. Each known variant names its text; `Other` holds the rest.
macro_rules! open_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $( $(#[$vmeta:meta])* $variant:ident => $text:literal, )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone)]
        $vis enum $name {
            $( $(#[$vmeta])* $variant, )*
            /// A value this version doesn't know, kept as written.
            Other(String),
        }

        impl $name {
            /// The value's text, as it is written in a manifest.
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $text, )*
                    Self::Other(text) => text,
                }
            }

            /// The value written as `text`: a known variant when there is one.
            pub fn parse(text: &str) -> Self {
                match text {
                    $( $text => Self::$variant, )*
                    other => Self::Other(other.to_string()),
                }
            }

            /// Whether this version knows the value.
            pub fn is_known(&self) -> bool {
                !matches!(Self::parse(self.as_str()), Self::Other(_))
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl Eq for $name {}

        impl PartialOrd for $name {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for $name {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.as_str().cmp(other.as_str())
            }
        }

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.as_str().hash(state)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Ok(Self::parse(&text))
            }
        }
    };
}

pub(crate) use open_enum;

#[cfg(test)]
mod tests {
    open_enum! {
        enum Colour {
            Red => "red",
            Blue => "blue",
        }
    }

    #[test]
    fn known_and_other_values() {
        assert_eq!(Colour::parse("red"), Colour::Red);
        assert!(Colour::Red.is_known());
        let other = Colour::parse("green");
        assert_eq!(other, Colour::Other("green".into()));
        assert!(!other.is_known());
        // An `Other` spelled like a known value is that value.
        assert_eq!(Colour::Other("blue".into()), Colour::Blue);
        assert!(Colour::Other("blue".into()).is_known());
    }

    #[test]
    fn serde_keeps_unknown_text() {
        let parsed: Vec<Colour> = serde_json::from_str(r#"["blue","Ultra violet"]"#).unwrap();
        assert_eq!(parsed[0], Colour::Blue);
        assert_eq!(
            serde_json::to_string(&parsed).unwrap(),
            r#"["blue","Ultra violet"]"#
        );
    }

    #[test]
    fn order_is_by_text() {
        let mut values = [Colour::Red, Colour::parse("amber"), Colour::Blue];
        values.sort();
        let texts: Vec<&str> = values.iter().map(|c| c.as_str()).collect();
        assert_eq!(texts, ["amber", "blue", "red"]);
    }
}
