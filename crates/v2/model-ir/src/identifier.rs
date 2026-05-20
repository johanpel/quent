/// A checked identifier adhering to the grammar `[A-Za-z][A-Za-z0-9_]*`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentifierError {
    #[error("must not be empty")]
    Empty,
    #[error("must start with an ASCII letter, found {0:?}")]
    InvalidStart(char),
    #[error("character {ch:?} at byte offset {index} is not [A-Za-z0-9_]")]
    InvalidChar { ch: char, index: usize },
}

impl Identifier {
    pub fn try_new(s: impl Into<String>) -> Result<Self, IdentifierError> {
        let s = s.into();
        let mut chars = s.char_indices();
        let (_, first) = chars.next().ok_or(IdentifierError::Empty)?;
        if !first.is_ascii_alphabetic() {
            return Err(IdentifierError::InvalidStart(first));
        }
        for (index, ch) in chars {
            if !(ch.is_ascii_alphanumeric() || ch == '_') {
                return Err(IdentifierError::InvalidChar { ch, index });
            }
        }
        Ok(Self(s))
    }

    pub fn new_unchecked(s: impl Into<String>) -> Self {
        Self::try_new(s).unwrap()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for Identifier {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Identifier {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for Identifier {
    type Err = IdentifierError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_new(s)
    }
}

impl TryFrom<&str> for Identifier {
    type Error = IdentifierError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl TryFrom<String> for Identifier {
    type Error = IdentifierError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn accepts_single_letter() {
        assert_eq!(Identifier::try_new("a").unwrap().as_str(), "a");
        assert_eq!(Identifier::try_new("Z").unwrap().as_str(), "Z");
    }

    #[test]
    fn accepts_letters_digits_underscores() {
        for s in ["foo", "Foo", "fooBar", "foo_bar", "foo123", "a_1_b_2", "x_"] {
            assert!(Identifier::try_new(s).is_ok(), "expected {s:?} to be valid");
        }
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(Identifier::try_new(""), Err(IdentifierError::Empty));
    }

    #[test]
    fn rejects_leading_digit() {
        assert_eq!(
            Identifier::try_new("1foo"),
            Err(IdentifierError::InvalidStart('1'))
        );
    }

    #[test]
    fn rejects_leading_underscore() {
        assert_eq!(
            Identifier::try_new("_foo"),
            Err(IdentifierError::InvalidStart('_'))
        );
        assert_eq!(
            Identifier::try_new("_"),
            Err(IdentifierError::InvalidStart('_'))
        );
    }

    #[test]
    fn rejects_leading_non_ascii_letter() {
        assert_eq!(
            Identifier::try_new("café"),
            Err(IdentifierError::InvalidChar { ch: 'é', index: 3 })
        );
        assert_eq!(
            Identifier::try_new("über"),
            Err(IdentifierError::InvalidStart('ü'))
        );
    }

    #[test]
    fn rejects_interior_punctuation() {
        assert_eq!(
            Identifier::try_new("foo-bar"),
            Err(IdentifierError::InvalidChar { ch: '-', index: 3 })
        );
        assert_eq!(
            Identifier::try_new("foo bar"),
            Err(IdentifierError::InvalidChar { ch: ' ', index: 3 })
        );
        assert_eq!(
            Identifier::try_new("foo.bar"),
            Err(IdentifierError::InvalidChar { ch: '.', index: 3 })
        );
        assert_eq!(
            Identifier::try_new("foo$bar"),
            Err(IdentifierError::InvalidChar { ch: '$', index: 3 })
        );
    }

    #[test]
    fn rejects_interior_non_ascii() {
        assert_eq!(
            Identifier::try_new("fooébar"),
            Err(IdentifierError::InvalidChar { ch: 'é', index: 3 })
        );
    }

    #[test]
    fn display_round_trips() {
        let id = Identifier::try_new("foo_42").unwrap();
        assert_eq!(id.to_string(), "foo_42");
    }

    #[test]
    fn deref_and_as_ref_expose_str() {
        let id = Identifier::try_new("foo").unwrap();
        let s: &str = &id;
        assert_eq!(s, "foo");
        assert_eq!(id.as_ref() as &str, "foo");
        assert_eq!(id.len(), 3);
    }

    #[test]
    fn into_string_returns_inner() {
        let id = Identifier::try_new("foo").unwrap();
        assert_eq!(id.into_string(), String::from("foo"));
    }

    #[test]
    fn from_str_and_try_from_agree_with_new() {
        assert_eq!(Identifier::from_str("foo"), Identifier::try_new("foo"));
        assert_eq!(Identifier::try_from("foo"), Identifier::try_new("foo"));
        assert_eq!(
            Identifier::try_from(String::from("foo")),
            Identifier::try_new("foo")
        );
        assert_eq!(Identifier::from_str("1foo"), Identifier::try_new("1foo"));
    }

    #[test]
    fn usable_as_hashmap_key_via_borrow() {
        use std::collections::HashMap;
        let mut m: HashMap<Identifier, u32> = HashMap::new();
        m.insert(Identifier::try_new("foo").unwrap(), 1);
        assert_eq!(m.get("foo"), Some(&1));
    }
}
